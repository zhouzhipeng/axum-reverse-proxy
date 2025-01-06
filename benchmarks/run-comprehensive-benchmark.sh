#!/bin/bash
set -euo pipefail

# Configuration
DURATION=30  # seconds per test
WARMUP=10    # seconds warmup before each test
OUTPUT_DIR="benchmarks/results"

# Get service IPs
AXUM_IP=$(kubectl get svc -n axum-benchmarks axum-proxy -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
NGINX_IP=$(kubectl get svc -n axum-benchmarks nginx-proxy -o jsonpath='{.status.loadBalancer.ingress[0].ip}')

# Create results directory
mkdir -p "$OUTPUT_DIR"
DATE=$(date +%Y%m%d_%H%M%S)
RESULT_FILE="$OUTPUT_DIR/benchmark_$DATE.md"

# Write header
cat << EOF > "$RESULT_FILE"
# Proxy Benchmark Results
Date: $(date)

## Environment
- GKE Node Type: e2-standard-2
- Node Count: 4
- Region: us-central1-a
- Backend: HTTP Echo Server
- Load Generator: wrk

## Test Scenarios
EOF

run_benchmark() {
    local name=$1
    local ip=$2
    local connections=$3
    local duration=$4
    local threads=$5
    local endpoint=$6
    local description=$7

    echo "Running $description for $name..."
    
    # Warmup
    kubectl exec -n axum-benchmarks load-tester -- wrk -t$threads -c$connections -d${WARMUP}s http://$ip:80$endpoint >/dev/null 2>&1
    
    # Actual test
    local result
    result=$(kubectl exec -n axum-benchmarks load-tester -- wrk -t$threads -c$connections -d${duration}s --latency http://$ip:80$endpoint)
    
    # Extract and format results
    local rps=$(echo "$result" | grep "Requests/sec" | awk '{print $2}')
    local latency_50=$(echo "$result" | grep "50%" | awk '{print $2}')
    local latency_75=$(echo "$result" | grep "75%" | awk '{print $2}')
    local latency_90=$(echo "$result" | grep "90%" | awk '{print $2}')
    local latency_99=$(echo "$result" | grep "99%" | awk '{print $2}')
    
    cat << EOF >> "$RESULT_FILE"

### $description - $name
\`\`\`
Connections: $connections
Threads: $threads
Duration: ${duration}s
Endpoint: $endpoint

Throughput: $rps req/sec
Latency:
  p50: $latency_50
  p75: $latency_75
  p90: $latency_90
  p99: $latency_99
\`\`\`

$(echo "$result" | grep -A 5 "Thread Stats")
EOF
}

# Function to run a complete test suite for a proxy
test_proxy() {
    local name=$1
    local ip=$2
    
    # Basic HTTP Tests - Different Concurrency Levels
    for conn in 10 100 1000; do
        run_benchmark "$name" "$ip" "$conn" "$DURATION" "2" "/" "Basic HTTP - $conn connections"
    done
    
    # Different Payload Sizes
    for size in 1 100 1000; do
        run_benchmark "$name" "$ip" "100" "$DURATION" "2" "/?size=${size}k" "Payload ${size}KB"
    done
    
    # Long-running Connections
    run_benchmark "$name" "$ip" "100" "120" "2" "/slow" "Long-running Connections"
    
    # Burst Mode (short duration, high concurrency)
    run_benchmark "$name" "$ip" "1000" "5" "4" "/" "Burst Mode"
}

echo "Starting comprehensive benchmark suite..."

# Test both proxies
for proxy in "Axum:$AXUM_IP" "Nginx:$NGINX_IP"; do
    NAME=${proxy%:*}
    IP=${proxy#*:}
    
    echo "Testing $NAME proxy at http://$IP:80"
    echo "-------------------"
    
    test_proxy "$NAME" "$IP"
done

# Generate summary
cat << EOF >> "$RESULT_FILE"

## Summary
The benchmark results show:

1. **Basic HTTP Performance**
   - Comparison of throughput and latency at different concurrency levels
   - Analysis of connection handling efficiency

2. **Payload Size Impact**
   - How each proxy handles different payload sizes
   - Memory efficiency and throughput characteristics

3. **Long-running Connections**
   - Stability and resource usage over time
   - Connection management effectiveness

4. **Burst Handling**
   - How well each proxy handles sudden traffic spikes
   - Recovery and stability characteristics

## Conclusions
(To be filled in after analyzing the results)

## Next Steps
- Add WebSocket benchmarks
- Test with HTTP/2
- Add backend failure scenarios
- Measure resource utilization
EOF

echo "Benchmark complete! Results written to $RESULT_FILE" 