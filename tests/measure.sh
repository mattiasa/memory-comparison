#!/usr/bin/env bash
# Start each implementation, scrape it SCRAPES times, then report its RSS.
# Uses the real /proc/net/tcp when present, otherwise the sample ./net_tcp.
set -euo pipefail
cd "$(dirname "$0")"

SCRAPES=${SCRAPES:-100}
BASE_PORT=${BASE_PORT:-9700}
if [[ -z "${PROC_NET_TCP:-}" ]]; then
  if [[ -r /proc/net/tcp ]]; then PROC_NET_TCP=/proc/net/tcp; else PROC_NET_TCP=$PWD/net_tcp; fi
fi
export PROC_NET_TCP

pid=
trap '[[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true' EXIT

measure() {
  local name=$1 port=$2; shift 2
  "$@" "$port" 2>/dev/null &
  pid=$!

  # Wait up to 5s for the server to come up.
  local metric=
  for _ in $(seq 50); do
    metric=$(curl -sf "localhost:$port/metrics" | awk '$1=="open_tcp_connections"{print $2}') && [[ -n "$metric" ]] && break
    kill -0 "$pid" 2>/dev/null || { echo "$name: exited before serving" >&2; return 1; }
    sleep 0.1
  done
  [[ -n "$metric" ]] || { echo "$name: no response on port $port" >&2; kill "$pid"; return 1; }

  for _ in $(seq "$SCRAPES"); do curl -sf -o /dev/null "localhost:$port/metrics"; done
  local rss
  rss=$(ps -o rss= -p "$pid" | tr -d ' ')
  kill "$pid"; wait "$pid" 2>/dev/null || true
  pid=

  printf '%-8s %10s KB %10s\n' "$name" "$rss" "$metric"
}

echo "file: $PROC_NET_TCP, scrapes per server: $SCRAPES"
printf '%-8s %13s %10s\n' impl RSS metric
measure rust   $((BASE_PORT + 0)) ../rust/target/release/tcp-exporter
measure go     $((BASE_PORT + 1)) ../go/tcp-exporter
measure python $((BASE_PORT + 2)) python3 ../python/tcp_exporter.py
