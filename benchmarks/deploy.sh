#!/bin/bash
set -euo pipefail

# Configuration
PROJECT_ID=$(gcloud config get-value project)
REGION=us-central1
ZONE=${REGION}-a
CLUSTER_NAME=axum-proxy-benchmarks
CLUSTER_SIZE=4

# Function to handle cleanup
cleanup() {
    if [ "${1:-}" = "--cleanup" ]; then
        echo "Cleaning up resources..."
        kubectl delete namespace axum-benchmarks
    fi
}

# Clean up any existing resources
echo "Cleaning up any existing resources..."
kubectl delete namespace axum-benchmarks --ignore-not-found

# Check if cluster exists
if ! gcloud container clusters describe $CLUSTER_NAME --zone $ZONE &>/dev/null; then
  echo "Creating GKE cluster..."
  gcloud container clusters create $CLUSTER_NAME \
    --zone $ZONE \
    --num-nodes $CLUSTER_SIZE \
    --machine-type e2-standard-2 \
    --enable-autoscaling \
    --min-nodes 2 \
    --max-nodes 6
else
  echo "Cluster already exists, getting credentials..."
  gcloud container clusters get-credentials $CLUSTER_NAME --zone $ZONE
fi

echo "Building Axum proxy image..."
docker buildx build --platform linux/amd64 -t axum-proxy -f benchmarks/k8s/Dockerfile.axum .

echo "Tagging and pushing to GCR..."
docker tag axum-proxy gcr.io/$PROJECT_ID/axum-proxy:latest
docker push gcr.io/$PROJECT_ID/axum-proxy:latest

echo "Updating image in deployment manifest..."
sed -i.bak "s|image: axum-proxy:latest|image: gcr.io/$PROJECT_ID/axum-proxy:latest|" benchmarks/k8s/01-axum-proxy.yaml

echo "Applying Kubernetes manifests..."
kubectl apply -f benchmarks/k8s/00-base.yaml
kubectl apply -f benchmarks/k8s/01-axum-proxy.yaml
kubectl apply -f benchmarks/k8s/02-nginx-proxy.yaml
kubectl apply -f benchmarks/k8s/03-load-tester.yaml

echo "Waiting for services to get external IPs..."
kubectl wait --namespace axum-benchmarks \
  --for=condition=ready pod \
  --selector=app=backend-service \
  --timeout=300s

kubectl wait --namespace axum-benchmarks \
  --for=condition=ready pod \
  --selector=app=axum-proxy \
  --timeout=300s

kubectl wait --namespace axum-benchmarks \
  --for=condition=ready pod \
  --selector=app=nginx-proxy \
  --timeout=300s

echo "Setup complete! Service endpoints:"
echo "Axum Proxy: $(kubectl get svc -n axum-benchmarks axum-proxy -o jsonpath='{.status.loadBalancer.ingress[0].ip}')"
echo "Nginx Proxy: $(kubectl get svc -n axum-benchmarks nginx-proxy -o jsonpath='{.status.loadBalancer.ingress[0].ip}')"

# Handle cleanup if requested
cleanup "$@" 