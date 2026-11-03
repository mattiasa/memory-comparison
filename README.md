This is a tiny project to measure the memory usage of a small metrics webserver
implemented in Rust, Go and Python.

## What the server does

Each implementation behaves the same way:

- Starts an HTTP server on a configurable port.
- Every 10 seconds, counts the entries in `/proc/net/tcp` (lines minus the
  header) and keeps the latest value in memory. One sample is also taken at
  startup so the first scrape isn't 0.
- Serves that value on `GET /metrics` as a Prometheus gauge. Any other path
  returns 404.

```
# HELP open_tcp_connections Number of entries in /proc/net/tcp.
# TYPE open_tcp_connections gauge
open_tcp_connections 24
```

The file is read in fixed 8 KB chunks, so memory use doesn't grow with the
number of sockets.

Note that `/proc/net/tcp` lists every IPv4 TCP socket, including listening
sockets and ones in `TIME_WAIT`, not only established connections. IPv6 sockets
are in `/proc/net/tcp6` and are not counted.

## Implementations

| Directory | Language | Dependencies |
|-----------|----------|--------------|
| `rust/`   | Rust     | [`tiny_http`](https://crates.io/crates/tiny_http) (synchronous, no async runtime) |
| `go/`     | Go       | standard library only (`net/http`) |
| `python/` | Python 3 | standard library only (`http.server`) |

Rust and Python handle requests one at a time. Go's `net/http` handles each
request on its own goroutine.

## Configuration

| Setting      | How to set it                                    | Default         |
|--------------|--------------------------------------------------|-----------------|
| Port         | first command-line argument, then `$PORT`        | `8080`          |
| File to read | `$PROC_NET_TCP`                                  | `/proc/net/tcp` |

`PROC_NET_TCP` exists so the servers can be run on machines without `/proc`,
such as macOS.

## Building and running

Requires `cargo`, `go` and `python3`.

```
make            # build all three
make rust       # or build one at a time
make go
make python     # nothing to compile; checks the script parses
make clean
```

```
./rust/target/release/tcp-exporter 9100
./go/tcp-exporter 9100
python3 python/tcp_exporter.py 9100

curl localhost:9100/metrics
```

## Measuring memory

```
make measure
make measure SCRAPES=1000
```

This builds everything, then runs `tests/measure.sh`, which starts each server
in turn, waits for it to respond, scrapes it `SCRAPES` times (default 100) and
reports its resident set size (RSS) from `ps`. It reads the real
`/proc/net/tcp` when it exists, and otherwise falls back to the sample file
`tests/net_tcp`. `BASE_PORT` (default 9700) sets the ports used.

### Results

macOS, Apple Silicon, 100 scrapes, against `tests/net_tcp`:

| Implementation | RSS      | Artifact size |
|----------------|----------|---------------|
| Rust           | ~2.4 MB  | 430 KB binary |
| Go             | ~11.6 MB | 5.5 MB binary (stripped) |
| Python 3.15    | ~21.8 MB | needs the interpreter installed |

Things to keep in mind when reading these numbers:

- They are from macOS. Linux typically reports lower RSS, especially for Go, so
  measure on the platform you'll deploy to.
- Most of Go's footprint is its runtime and garbage collector. Setting
  `GOMAXPROCS=1` and a low `GOMEMLIMIT` reduces it somewhat.
- About 10 MB of Python's footprint is the interpreter itself.
- RSS is a single snapshot after the scrapes, not a peak.
