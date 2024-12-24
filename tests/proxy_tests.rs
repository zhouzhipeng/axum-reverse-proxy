use axum::{
    extract::Json,
    routing::{delete, get, post, put},
    Router,
};
use rproxy::ReverseProxy;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{trace, Level};
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

// Helper function to create a test server
async fn create_test_server() -> SocketAddr {
    // Create a router with test endpoints
    let app = Router::new()
        .route("/echo", get(echo_json_handler))
        .route("/echo", post(echo_json_handler))
        .route("/echo", put(echo_json_handler))
        .route("/echo", delete(echo_json_handler))
        .route("/json", get(json_handler))
        .route("/delay/:seconds", get(delay_handler));

    // Find a random available port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn the server in the background
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    addr
}

// Test handlers
async fn echo_json_handler(Json(body): Json<Value>) -> Json<Value> {
    Json(body)
}

async fn json_handler() -> Json<Value> {
    Json(json!({
        "status": "success",
        "message": "Hello from test server!"
    }))
}

async fn delay_handler(axum::extract::Path(seconds): axum::extract::Path<u64>) -> &'static str {
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    "Done!"
}

#[tokio::test]
async fn test_proxy_get_request() {
    init_tracing();
    // Start the test server
    let server_addr = create_test_server().await;

    // Create and start the proxy server
    let proxy = ReverseProxy::new(&format!("http://{}", server_addr));
    let app = proxy.router();
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Give servers time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Make a request through the proxy
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/json", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "success");
    assert_eq!(body["message"], "Hello from test server!");
}

#[tokio::test]
async fn test_proxy_post_request() {
    init_tracing();
    let server_addr = create_test_server().await;

    let proxy = ReverseProxy::new(&format!("http://{}", server_addr));
    let app = proxy.router();
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test POST request with JSON body
    let client = reqwest::Client::new();
    let test_body = json!({"test": "data"});

    let response = client
        .post(format!("http://{}/echo", proxy_addr))
        .json(&test_body)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["test"], "data");
}

#[tokio::test]
async fn test_proxy_large_payload() {
    init_tracing();
    let server_addr = create_test_server().await;

    let proxy = ReverseProxy::new(&format!("http://{}", server_addr));
    let app = proxy.router();
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create a large payload (1MB of JSON data)
    let large_data = "x".repeat(1024 * 1024);
    let test_body = json!({ "data": large_data });

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/echo", proxy_addr))
        .json(&test_body)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["data"].as_str().unwrap().len(), 1024 * 1024);
}

#[tokio::test]
async fn test_proxy_timeout() {
    init_tracing();
    let server_addr = create_test_server().await;

    let proxy = ReverseProxy::new(&format!("http://{}", server_addr));
    let app = proxy.router();
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Make a request that should trigger a timeout
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/delay/5", proxy_addr))
        .timeout(Duration::from_secs(1))
        .send()
        .await;

    assert!(response.is_err(), "Expected timeout error");
}

#[tokio::test]
async fn test_proxy_http2_support() {
    init_tracing();

    // Create a test server with HTTP/2 support
    let server_addr = create_test_server().await;

    // Create a proxy to the test server
    let proxy = ReverseProxy::new(&format!("http://{}", server_addr));
    let app = proxy.router();
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create a client that uses HTTP/2
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap();

    let response = client
        .get(format!("http://{}/json", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    trace!(version = ?response.version(), "Response version");

    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "success");
    assert_eq!(body["message"], "Hello from test server!");
}
