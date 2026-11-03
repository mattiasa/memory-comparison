// tcp-exporter: serves the number of entries in /proc/net/tcp as a
// Prometheus gauge. Standard library only.
package main

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strconv"
	"sync/atomic"
	"time"
)

const (
	defaultPort = 8080
	interval    = 10 * time.Second
)

// countEntries counts lines minus the header using a fixed buffer, so memory
// use doesn't grow with the number of sockets.
func countEntries(path string, buf []byte) (uint64, error) {
	f, err := os.Open(path)
	if err != nil {
		return 0, err
	}
	defer f.Close()
	var lines uint64
	for {
		n, err := f.Read(buf)
		lines += uint64(bytes.Count(buf[:n], []byte{'\n'}))
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return 0, err
		}
	}
	if lines == 0 {
		return 0, nil
	}
	return lines - 1, nil
}

func main() {
	// Port: first CLI argument, then $PORT, then the default.
	port := defaultPort
	s := os.Getenv("PORT")
	if len(os.Args) > 1 {
		s = os.Args[1]
	}
	if s != "" {
		p, err := strconv.ParseUint(s, 10, 16)
		if err != nil {
			fmt.Fprintf(os.Stderr, "invalid port: %s\n", s)
			os.Exit(2)
		}
		port = int(p)
	}
	path := os.Getenv("PROC_NET_TCP")
	if path == "" {
		path = "/proc/net/tcp"
	}

	var count atomic.Uint64
	buf := make([]byte, 8192)
	sample := func() {
		if n, err := countEntries(path, buf); err != nil {
			log.Printf("failed to read %s: %v", path, err)
		} else {
			count.Store(n)
		}
	}
	// Take one sample up front so the first scrape isn't a misleading 0.
	sample()
	go func() {
		for range time.Tick(interval) {
			sample()
		}
	}()

	http.HandleFunc("/metrics", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain; version=0.0.4")
		fmt.Fprintf(w, "# HELP open_tcp_connections Number of entries in /proc/net/tcp.\n"+
			"# TYPE open_tcp_connections gauge\n"+
			"open_tcp_connections %d\n", count.Load())
	})

	addr := fmt.Sprintf("0.0.0.0:%d", port)
	log.Printf("listening on %s, serving /metrics", addr)
	log.Fatal(http.ListenAndServe(addr, nil))
}
