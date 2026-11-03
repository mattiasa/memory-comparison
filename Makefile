.PHONY: all rust go python measure clean

SCRAPES ?= 100

all: rust go python

# cargo and go do their own up-to-date checks, so these always delegate.
rust:
	cd rust && cargo build --release

go:
	cd go && go build -ldflags="-s -w" -trimpath -o tcp-exporter .

# Nothing to compile; just check the script parses.
python:
	python3 -m py_compile python/tcp_exporter.py

# Builds first, then reports RSS of each server after SCRAPES requests.
# Override with e.g. make measure SCRAPES=1000
measure: all
	SCRAPES=$(SCRAPES) ./tests/measure.sh

clean:
	cd rust && cargo clean
	rm -f go/tcp-exporter
	rm -rf python/__pycache__
