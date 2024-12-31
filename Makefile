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

PYTHON_VERSIONS =  cp38-cp38 cp39-cp39 cp310-cp310 cp311-cp311 cp312-cp312 cp313-cp313 cp313-cp313t

build-wheels:
	for PLATFORM in x86_64 aarch64; do \
		$(DOCKER) run \
			--env PYTHON_VERSIONS='$(PYTHON_VERSIONS)' \
			--platform linux/$${PLATFORM} \
			--rm \
			-v $$PWD:/io:rw quay.io/pypa/manylinux_2_34_$${PLATFORM}:2024.12.28-1 \
			-- \
			bash -xe -c 'curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; . ~/.cargo/env; for PYTHON_VERSION in $${PYTHON_VERSIONS}; do export PYTHON_ROOT="/opt/python/$${PYTHON_VERSION}"; (export PATH="$${PYTHON_ROOT}/bin:$${PATH}"; pip install maturin; cd /io; maturin build --release); done'; \
	done
