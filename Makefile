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
	$(DOCKER) run --env PYTHON_ROOT="/opt/python/$${PYTHON}/bin" --rm -v $$PWD:/io:rw pypa-manylinux2014-with-rustup:2023-06-11-02cacaf -- sh -c '. ~/.cargo/env && cd /io && "$${PYTHON_ROOT}/maturin" build --release'
