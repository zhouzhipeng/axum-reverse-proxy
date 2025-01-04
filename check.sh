#!/bin/bash
set -e

echo "Running cargo fmt..."
cargo fmt --all

echo "Running cargo check..."
cargo check

echo "Running clippy..."
cargo clippy --fix --allow-dirty --allow-staged -- -D warnings
cargo clippy --bench proxy_bench --fix --allow-dirty --allow-staged -- -D warnings
cargo clippy --bench websocket_bench --fix --allow-dirty --allow-staged -- -D warnings

echo "Running tests..."
cargo test 