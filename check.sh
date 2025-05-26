#!/bin/bash
set -e

echo "Running cargo fmt..."
cargo fmt --all

echo "Running cargo check..."
cargo check --features full

echo "Running clippy..."
cargo clippy --features full --fix --allow-dirty --allow-staged -- -D warnings
cargo clippy --features full --bench proxy_bench --fix --allow-dirty --allow-staged -- -D warnings
cargo clippy --features full --bench websocket_bench --fix --allow-dirty --allow-staged -- -D warnings

echo "Running tests..."
cargo test --features full 