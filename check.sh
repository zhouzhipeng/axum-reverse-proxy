#!/bin/bash
set -e

echo "Running cargo fmt..."
cargo fmt --all -- --check

echo "Running cargo check..."
cargo check

echo "Running clippy..."
cargo clippy -- -D warnings
cargo clippy --bench proxy_bench -- -D warnings
cargo clippy --bench websocket_bench -- -D warnings

echo "Running tests..."
cargo test 