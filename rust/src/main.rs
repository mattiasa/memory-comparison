use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tiny_http::{Header, Response, Server};

const DEFAULT_PORT: u16 = 8080;
const INTERVAL: Duration = Duration::from_secs(10);

/// Counts entries in /proc/net/tcp (lines minus the header line) using a
/// fixed stack buffer, so memory use doesn't grow with the number of sockets.
fn count_entries(path: &str) -> io::Result<u64> {
    let mut file = File::open(path)?;
    let mut buf = [0u8; 8192];
    let mut lines = 0u64;
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        lines += buf[..n].iter().filter(|&&b| b == b'\n').count() as u64;
    }
    Ok(lines.saturating_sub(1))
}

fn main() {
    // Port: first CLI argument, then $PORT, then the default.
    let port: u16 = env::args()
        .nth(1)
        .or_else(|| env::var("PORT").ok())
        .map(|s| s.parse().unwrap_or_else(|_| {
            eprintln!("invalid port: {s}");
            std::process::exit(2);
        }))
        .unwrap_or(DEFAULT_PORT);
    let path = env::var("PROC_NET_TCP").unwrap_or_else(|_| "/proc/net/tcp".into());

    let count = Arc::new(AtomicU64::new(0));
    // Take one sample up front so the first scrape isn't a misleading 0.
    match count_entries(&path) {
        Ok(n) => count.store(n, Ordering::Relaxed),
        Err(e) => eprintln!("failed to read {path}: {e}"),
    }

    {
        let count = Arc::clone(&count);
        let path = path.clone();
        thread::Builder::new()
            .name("sampler".into())
            .stack_size(64 * 1024)
            .spawn(move || loop {
                thread::sleep(INTERVAL);
                match count_entries(&path) {
                    Ok(n) => count.store(n, Ordering::Relaxed),
                    Err(e) => eprintln!("failed to read {path}: {e}"),
                }
            })
            .expect("failed to spawn sampler thread");
    }

    let server = Server::http(("0.0.0.0", port)).unwrap_or_else(|e| {
        eprintln!("failed to bind port {port}: {e}");
        std::process::exit(1);
    });
    eprintln!("listening on 0.0.0.0:{port}, serving /metrics");

    let content_type =
        Header::from_bytes(&b"Content-Type"[..], &b"text/plain; version=0.0.4"[..]).unwrap();

    // Requests are handled one at a time on the main thread; scrapes are tiny.
    for request in server.incoming_requests() {
        let response = if request.url() == "/metrics" {
            let body = format!(
                "# HELP open_tcp_connections Number of entries in /proc/net/tcp.\n\
                 # TYPE open_tcp_connections gauge\n\
                 open_tcp_connections {}\n",
                count.load(Ordering::Relaxed)
            );
            Response::from_string(body).with_header(content_type.clone())
        } else {
            Response::from_string("not found\n").with_status_code(404)
        };
        let _ = request.respond(response);
    }
}
