use axum::{routing::get, Router};
use axum_reverse_proxy::{RetryLayer, ReverseProxy};
use std::time::Duration;
use tokio::net::TcpListener;
use tower::ServiceBuilder;

async fn delayed_server(addr: std::net::SocketAddr) {
    tokio::time::sleep(Duration::from_millis(100)).await;
    let app = Router::new().route("/test", get(|| async { "ok" }));
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[tokio::test]
async fn test_no_retry_by_default() {
    // Reserve an address and then release it
    let temp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = temp.local_addr().unwrap();
    drop(temp);

    let proxy = ReverseProxy::new("/", &format!("http://{}", addr));
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Spawn server after delay
    let server_handle = tokio::spawn(delayed_server(addr));

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/test", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_GATEWAY);

    proxy_server.abort();
    server_handle.abort();
}

#[tokio::test]
async fn test_retry_layer() {
    let temp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = temp.local_addr().unwrap();
    drop(temp);

    let proxy = ReverseProxy::new("/", &format!("http://{}", addr));
    let app: Router = proxy.into();
    let app = app.layer(ServiceBuilder::new().layer(RetryLayer::new(5)));

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    let server_handle = tokio::spawn(delayed_server(addr));

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/test", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    proxy_server.abort();
    server_handle.abort();
}

#[tokio::test]
async fn test_retry_layer_zero_attempts() {
    let temp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = temp.local_addr().unwrap();
    drop(temp);

    let proxy = ReverseProxy::new("/", &format!("http://{}", addr));
    let app: Router = proxy.into();
    let app = app.layer(ServiceBuilder::new().layer(RetryLayer::new(0)));

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    let server_handle = tokio::spawn(delayed_server(addr));

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/test", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_GATEWAY);

    proxy_server.abort();
    server_handle.abort();
}
