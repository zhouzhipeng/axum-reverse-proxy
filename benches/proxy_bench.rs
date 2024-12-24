use axum::{
    extract::Json,
    routing::{get, post},
    Router,
};
use axum_reverse_proxy::ReverseProxy;
use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc};
use tokio::time::Duration;
use tokio::{net::TcpListener, sync::Notify, task::JoinHandle};
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

fn init_tracing() {
    let _ = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .try_init();
}

struct TestServer {
    addr: SocketAddr,
    _notify: Arc<Notify>,
    _handle: JoinHandle<()>,
}

// Helper function to create a test server
async fn create_test_server() -> TestServer {
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/json", get(json_handler))
        .route("/echo", post(echo_json_handler));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    info!("Test server binding to {}", addr);

    // Create a notify to keep the server alive
    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();

    let handle = tokio::spawn(async move {
        info!("Test server starting");
        let server = axum::serve(listener, app);
        tokio::select! {
            result = server => {
                if let Err(e) = result {
                    error!("Test server error: {}", e);
                }
            }
            _ = notify_clone.notified() => {
                info!("Test server shutting down");
            }
        }
    });

    // Wait for the server to be ready
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(32)
        .build()
        .unwrap();

    let health_url = format!("http://{}/health", addr);
    for i in 0..10 {
        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("Test server is ready at {}", addr);
                return TestServer {
                    addr,
                    _notify: notify,
                    _handle: handle,
                };
            }
            Ok(resp) => {
                error!("Health check failed with status: {}", resp.status());
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(e) => {
                if i < 9 {
                    error!("Health check attempt {} failed: {}", i + 1, e);
                    tokio::time::sleep(Duration::from_millis(200)).await;
                } else {
                    panic!("Failed to connect to test server after 10 attempts: {}", e);
                }
            }
        }
    }
    panic!("Test server failed to become ready");
}

struct ProxyServer {
    addr: SocketAddr,
    _notify: Arc<Notify>,
    _handle: JoinHandle<()>,
}

// Helper function to create and verify proxy server
async fn create_proxy_server(target: String) -> ProxyServer {
    let proxy = ReverseProxy::new(&target);
    let app = proxy.router();
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = proxy_listener.local_addr().unwrap();
    info!("Proxy server binding to {}", addr);

    // Create a notify to keep the server alive
    let notify = Arc::new(Notify::new());
    let notify_clone = notify.clone();

    let handle = tokio::spawn(async move {
        info!("Proxy server starting");
        let server = axum::serve(proxy_listener, app);
        tokio::select! {
            result = server => {
                if let Err(e) = result {
                    error!("Proxy server error: {}", e);
                }
            }
            _ = notify_clone.notified() => {
                info!("Proxy server shutting down");
            }
        }
    });

    // Wait for the proxy to be ready
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(32)
        .build()
        .unwrap();

    let health_url = format!("http://{}/health", addr);
    for i in 0..10 {
        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("Proxy server is ready at {}", addr);
                return ProxyServer {
                    addr,
                    _notify: notify,
                    _handle: handle,
                };
            }
            Ok(resp) => {
                error!("Proxy health check failed with status: {}", resp.status());
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(e) => {
                if i < 9 {
                    error!("Proxy health check attempt {} failed: {}", i + 1, e);
                    tokio::time::sleep(Duration::from_millis(200)).await;
                } else {
                    panic!("Failed to connect to proxy server after 10 attempts: {}", e);
                }
            }
        }
    }
    panic!("Proxy server failed to become ready");
}

// Test handlers
async fn health_handler() -> &'static str {
    "OK"
}

async fn echo_json_handler(Json(body): Json<Value>) -> Json<Value> {
    Json(body)
}

async fn json_handler() -> Json<Value> {
    Json(json!({
        "status": "success",
        "message": "Hello from test server!"
    }))
}

// Benchmark HTTP/1.1 GET requests
async fn bench_http1_get(client: &reqwest::Client, proxy_addr: SocketAddr) {
    let url = format!("http://{}/json", proxy_addr);
    let response = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to send request to {}: {}", url, e);
            panic!("Request failed: {}", e);
        }
    };

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        error!("Unexpected status code {}: {}", status, text);
        panic!("Unexpected status code: {}", status);
    }

    let _body: Value = response.json().await.unwrap();
}

// Benchmark HTTP/2 GET requests
async fn bench_http2_get(client: &reqwest::Client, proxy_addr: SocketAddr) {
    let url = format!("http://{}/json", proxy_addr);
    let response = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to send request to {}: {}", url, e);
            panic!("Request failed: {}", e);
        }
    };

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        error!("Unexpected status code {}: {}", status, text);
        panic!("Unexpected status code: {}", status);
    }

    let _body: Value = response.json().await.unwrap();
}

// Benchmark large payload POST requests
async fn bench_large_payload(client: &reqwest::Client, proxy_addr: SocketAddr) {
    let large_data = "x".repeat(1024 * 1024); // 1MB payload
    let test_body = json!({ "data": large_data });

    let url = format!("http://{}/echo", proxy_addr);
    let response = match client.post(&url).json(&test_body).send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to send request to {}: {}", url, e);
            panic!("Request failed: {}", e);
        }
    };

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        error!("Unexpected status code {}: {}", status, text);
        panic!("Unexpected status code: {}", status);
    }

    let _body: Value = response.json().await.unwrap();
}

// Benchmark concurrent requests
async fn bench_concurrent_requests(client: &reqwest::Client, proxy_addr: SocketAddr) {
    let mut handles = Vec::new();
    for i in 0..10 {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            let url = format!("http://{}/json", proxy_addr);
            let response = match client.get(&url).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    error!("Failed to send request {} to {}: {}", i, url, e);
                    panic!("Request {} failed: {}", i, e);
                }
            };

            let status = response.status();
            if !status.is_success() {
                let text = response.text().await.unwrap_or_default();
                error!("Request {} unexpected status code {}: {}", i, status, text);
                panic!("Request {} unexpected status code: {}", i, status);
            }

            let _body: Value = response.json().await.unwrap();
        }));
    }
    for (i, handle) in handles.into_iter().enumerate() {
        if let Err(e) = handle.await {
            error!("Task {} failed: {}", i, e);
            panic!("Task {} failed: {}", i, e);
        }
    }
}

fn proxy_benchmark(c: &mut Criterion) {
    init_tracing();
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Set up the test environment
    let (test_server, proxy_server) = rt.block_on(async {
        let test_server = create_test_server().await;
        let proxy_server = create_proxy_server(format!("http://{}", test_server.addr)).await;
        (test_server, proxy_server)
    });

    info!("Test environment ready:");
    info!("  Test server: {}", test_server.addr);
    info!("  Proxy server: {}", proxy_server.addr);

    // Create HTTP/1.1 client with appropriate timeouts
    let http1_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(32)
        .build()
        .unwrap();

    // Create HTTP/2 client with appropriate timeouts
    let http2_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(32)
        .http2_prior_knowledge()
        .build()
        .unwrap();

    // Verify both servers are working before starting benchmarks
    rt.block_on(async {
        bench_http1_get(&http1_client, proxy_server.addr).await;
        info!("Initial HTTP/1.1 test successful");
        bench_http2_get(&http2_client, proxy_server.addr).await;
        info!("Initial HTTP/2 test successful");
    });

    // Benchmark HTTP/1.1 GET requests
    c.bench_function("http1_get", |b| {
        b.to_async(&rt)
            .iter(|| bench_http1_get(&http1_client, proxy_server.addr));
    });

    // Benchmark HTTP/2 GET requests
    c.bench_function("http2_get", |b| {
        b.to_async(&rt)
            .iter(|| bench_http2_get(&http2_client, proxy_server.addr));
    });

    // Benchmark large payload POST requests
    c.bench_function("large_payload_post", |b| {
        b.to_async(&rt)
            .iter(|| bench_large_payload(&http1_client, proxy_server.addr));
    });

    // Benchmark concurrent requests
    c.bench_function("concurrent_requests", |b| {
        b.to_async(&rt)
            .iter(|| bench_concurrent_requests(&http1_client, proxy_server.addr));
    });
}

criterion_group!(benches, proxy_benchmark);
criterion_main!(benches);
