# Axum Reverse Proxy Benchmarks

This directory contains the benchmark suite for comparing the performance of our Axum-based reverse proxy against Nginx.

## Setup

The benchmarks run on Google Kubernetes Engine (GKE) with the following configuration:
- Node type: e2-standard-2 (2 vCPU, 8GB memory)
- Node count: 4
- Region: us-central1-a

The test environment consists of:
1. A backend service (Rust/Axum) that can generate responses of various sizes
2. Two proxy deployments (Axum and Nginx) with identical configurations
3. A load generator pod running wrk2 for load testing

## Running Benchmarks

To run the benchmarks:

```bash
# Deploy the test environment
./deploy.sh

# Run the comprehensive benchmark suite
./run-comprehensive-benchmark.sh
```

## Test Scenarios

The benchmark suite tests:

1. **Basic HTTP Performance**
   - Tests with 10, 100, and 1000 concurrent connections
   - Measures throughput and latency distributions

2. **Payload Size Impact**
   - Tests with 1KB, 100KB, and 1MB response sizes
   - Measures how payload size affects performance

3. **Long-running Connections**
   - Tests with slow responses (1s delay)
   - Measures connection handling efficiency

4. **Burst Mode**
   - High concurrency, short duration tests
   - Measures how well proxies handle sudden load

## Results

Benchmark results are stored in two formats:
1. Markdown reports in `results/` with detailed metrics
2. JSON data in `results/data/` for analysis
3. Plots in `results/plots/` visualizing the results

The latest results are published to GitHub Pages at: https://[username].github.io/axum-reverse-proxy/

## Continuous Integration

Benchmarks run automatically on:
- Every push to main
- Every pull request
- Manual trigger via GitHub Actions

Results are uploaded as artifacts and published to GitHub Pages. 