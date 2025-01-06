#!/bin/bash
set -euo pipefail

# Get service IPs
AXUM_IP=$(kubectl get svc -n axum-benchmarks axum-proxy -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
NGINX_IP=$(kubectl get svc -n axum-benchmarks nginx-proxy -o jsonpath='{.status.loadBalancer.ingress[0].ip}')

echo "Running benchmarks..."
echo "===================="

for proxy in "Axum:$AXUM_IP" "Nginx:$NGINX_IP"; do
  NAME=${proxy%:*}
  IP=${proxy#*:}
  
  echo "Testing $NAME proxy at http://$IP:80"
  echo "-------------------"
  
  # Run wrk benchmark from the load-tester pod
  # -c: connections
  # -d: duration
  # -t: threads
  kubectl exec -n axum-benchmarks load-tester -- wrk -t2 -c100 -d30s --latency http://$IP:80/
  echo
done 