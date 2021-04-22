DOCKER = docker

source_files := $(wildcard src/*.rs)

all: tantivy/tantivy.so

PHONY: test format

test: tantivy/tantivy.so
	python3 -m pytest

format:
	rustfmt src/*.rs

tantivy/tantivy.so: target/debug/libtantivy.so
	cp target/debug/libtantivy.so tantivy/tantivy.so

target/debug/libtantivy.so: $(source_files)
	cargo build

build-wheels:
	$(DOCKER) run --rm -v $$PWD:/io:rw konstin2/maturin:latest build --release
