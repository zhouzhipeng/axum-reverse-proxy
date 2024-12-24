use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::{get, post},
    Router,
};
use axum_reverse_proxy::ReverseProxy;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_proxy_get_request() {
    // Create a test server
    let app = Router::new().route(
        "/test",
        get(|| async { Json(json!({"message": "Hello from test server!"})) }),
    );

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let test_server = tokio::spawn(async move {
        axum::serve(test_listener, app).await.unwrap();
    });

    // Create a reverse proxy
    let proxy = ReverseProxy::new(&format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Create a client
    let client = reqwest::Client::new();

    // Send a request through the proxy
    let response = client
        .get(&format!("http://{}/test", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "Hello from test server!");

    // Clean up
    proxy_server.abort();
    test_server.abort();
}

#[tokio::test]
async fn test_proxy_post_request() {
    // Create a test server
    let app = Router::new().route(
        "/echo",
        post(|body: Json<Value>| async move { Json(body.0) }),
    );

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let test_server = tokio::spawn(async move {
        axum::serve(test_listener, app).await.unwrap();
    });

    // Create a reverse proxy
    let proxy = ReverseProxy::new(&format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Create a client
    let client = reqwest::Client::new();

    // Send a request through the proxy
    let test_body = json!({"message": "Hello, proxy!"});
    let response = client
        .post(&format!("http://{}/echo", proxy_addr))
        .json(&test_body)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body, test_body);

    // Clean up
    proxy_server.abort();
    test_server.abort();
}

#[tokio::test]
async fn test_proxy_large_payload() {
    // Create a test server
    let app = Router::new().route(
        "/echo",
        post(|body: Json<Value>| async move { Json(body.0) }),
    );

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let test_server = tokio::spawn(async move {
        axum::serve(test_listener, app).await.unwrap();
    });

    // Create a reverse proxy
    let proxy = ReverseProxy::new(&format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Create a client
    let client = reqwest::Client::new();

    // Create a large payload (1MB)
    let large_data = "x".repeat(1024 * 1024);
    let test_body = json!({"data": large_data});

    // Send a request through the proxy
    let response = client
        .post(&format!("http://{}/echo", proxy_addr))
        .json(&test_body)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body, test_body);

    // Clean up
    proxy_server.abort();
    test_server.abort();
}

#[tokio::test]
async fn test_proxy_timeout() {
    // Create a test server
    let app = Router::new().route("/delay", get(|| async {
        tokio::time::sleep(Duration::from_secs(2)).await;
        "Delayed response"
    }));

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let test_server = tokio::spawn(async move {
        axum::serve(test_listener, app).await.unwrap();
    });

    // Create a reverse proxy
    let proxy = ReverseProxy::new(&format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Create a client with a short timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();

    // Send a request through the proxy
    let response = client
        .get(&format!("http://{}/delay", proxy_addr))
        .send()
        .await;

    assert!(response.is_err());
    assert!(response.unwrap_err().is_timeout());

    // Clean up
    proxy_server.abort();
    test_server.abort();
}

#[tokio::test]
async fn test_proxy_http2_support() {
    // Create a test server
    let app = Router::new().route(
        "/test",
        get(|| async { Json(json!({"message": "Hello from test server!"})) }),
    );

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let test_server = tokio::spawn(async move {
        axum::serve(test_listener, app).await.unwrap();
    });

    // Create a reverse proxy
    let proxy = ReverseProxy::new(&format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Create an HTTP/2 client
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap();

    // Send a request through the proxy
    let response = client
        .get(&format!("http://{}/test", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "Hello from test server!");

    // Clean up
    proxy_server.abort();
    test_server.abort();
}

#[tokio::test]
async fn test_proxy_nested_routing() {
    // Create a test server
    let app = Router::new().route(
        "/test",
        get(|| async { Json(json!({"message": "Hello from test server!"})) }),
    );

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let test_server = tokio::spawn(async move {
        axum::serve(test_listener, app).await.unwrap();
    });

    // Create a reverse proxy
    let proxy = ReverseProxy::new(&format!("http://{}", test_addr));
    let proxy_router: Router<()> = proxy.into();

    // Create an app state
    #[derive(Clone)]
    struct AppState {
        name: String,
    }
    let state = AppState {
        name: "test app".to_string(),
    };

    // Create a main router with app state and nested proxy
    let app = Router::new()
        .route(
            "/",
            get(|State(state): State<AppState>| async move {
                Json(json!({ "app": state.name }))
            }),
        )
        .nest_service("/proxy", proxy_router)
        .with_state(state);

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Create a client
    let client = reqwest::Client::new();

    // Test root endpoint with app state
    let response = client
        .get(&format!("http://{}/", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["app"], "test app");

    // Test proxied endpoint
    let response = client
        .get(&format!("http://{}/proxy/test", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "Hello from test server!");

    // Clean up
    proxy_server.abort();
    test_server.abort();
}
