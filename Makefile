.PHONY: all build check test lint fmt clean

all: check lint test build

build:
	cargo build --all

check:
	cargo check --all

test:
	cargo test --all

lint:
	cargo clippy --all -- -D warnings

fmt:
	cargo fmt --all --check

fmt-fix:
	cargo fmt --all

clean:
	cargo clean

run-client:
	cargo run -p speak-up-client

run-backend:
	cargo run -p speak-up-backend

check-core:
	cargo check -p speak-up-core

check-client:
	cargo check -p speak-up-client

check-backend:
	cargo check -p speak-up-backend
