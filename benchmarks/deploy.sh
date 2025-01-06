#!/bin/bash
set -euo pipefail

echo "Cleaning up any existing resources..."
kubectl delete namespace axum-benchmarks || true

echo "Cluster already exists, getting credentials..."
gcloud container clusters get-credentials axum-proxy-benchmarks --zone us-central1-a

echo "Building Axum proxy image..."
docker buildx build --platform linux/amd64 -f benchmarks/k8s/Dockerfile.axum -t axum-proxy .

echo "Building backend image..."
docker buildx build --platform linux/amd64 -f benchmarks/docker/backend/Dockerfile -t backend benchmarks/docker/backend

echo "Tagging and pushing to GCR..."
docker tag axum-proxy gcr.io/axum-proxy-benchmarks/axum-proxy:latest
docker tag backend gcr.io/axum-proxy-benchmarks/backend:latest

docker push gcr.io/axum-proxy-benchmarks/axum-proxy:latest
docker push gcr.io/axum-proxy-benchmarks/backend:latest

echo "Updating image in deployment manifest..."
# No need to update image names as they're now fixed in the manifests

echo "Applying Kubernetes manifests..."
kubectl apply -f benchmarks/k8s/

echo "Waiting for services to get external IPs..."
kubectl wait --namespace axum-benchmarks \
  --for=condition=ready pod \
  --selector=app=backend-service \
  --timeout=90s

kubectl wait --namespace axum-benchmarks \
  --for=condition=ready pod \
  --selector=app=axum-proxy \
  --timeout=90s

kubectl wait --namespace axum-benchmarks \
  --for=condition=ready pod \
  --selector=app=nginx-proxy \
  --timeout=90s

echo "Setup complete! Service endpoints:"
echo "Axum Proxy: $(kubectl get svc -n axum-benchmarks axum-proxy -o jsonpath='{.status.loadBalancer.ingress[0].ip}')"
echo "Nginx Proxy: $(kubectl get svc -n axum-benchmarks nginx-proxy -o jsonpath='{.status.loadBalancer.ingress[0].ip}')" 