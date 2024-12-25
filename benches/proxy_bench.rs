#![allow(unused_must_use)]

use axum::{
    routing::{get, post},
    Router,
};
use axum_reverse_proxy::{ProxyOptions, ReverseProxy};
use criterion::{criterion_group, criterion_main, Criterion};
use hyper::StatusCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Notify;

async fn create_test_server() -> String {
    // Create a test server
    let app = Router::new()
        .route("/test", get(|| async { "Hello from test server!" }))
        .route("/echo", post(|body: String| async move { body }));

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap().to_string();

    let server_ready = Arc::new(Notify::new());
    let server_ready_clone = server_ready.clone();

    tokio::spawn(async move {
        server_ready_clone.notify_one();
        axum::serve(test_listener, app).await.unwrap();
    });

    server_ready.notified().await;
    test_addr
}

fn bench_http1_get(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let test_addr = rt.block_on(create_test_server());

    // Streaming proxy
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(listener, app).await.unwrap();
        });

        server_ready.notified().await;
        addr
    });

    let client = reqwest::Client::new();

    c.bench_function("http1_get_streaming", |b| {
        b.to_async(&rt).iter(|| async {
            let response = client
                .get(format!("http://{}/test", proxy_listener))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
        });
    });

    // Buffered proxy
    let proxy = ReverseProxy::new_with_options(
        "/",
        &format!("http://{}", test_addr),
        ProxyOptions {
            buffer_bodies: true,
        },
    );
    let app: Router = proxy.into();

    let buffered_proxy_listener = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(listener, app).await.unwrap();
        });

        server_ready.notified().await;
        addr
    });

    c.bench_function("http1_get_buffered", |b| {
        b.to_async(&rt).iter(|| async {
            let response = client
                .get(format!("http://{}/test", buffered_proxy_listener))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
        });
    });
}

fn bench_http2_get(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let test_addr = rt.block_on(create_test_server());
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(listener, app).await.unwrap();
        });

        server_ready.notified().await;
        addr
    });

    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap();

    c.bench_function("http2_get", |b| {
        b.to_async(&rt).iter(|| async {
            let response = client
                .get(format!("http://{}/test", proxy_listener))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
        });
    });
}

fn bench_large_payload(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let test_addr = rt.block_on(create_test_server());

    // Streaming proxy
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(listener, app).await.unwrap();
        });

        server_ready.notified().await;
        addr
    });

    let client = reqwest::Client::new();
    let large_data = "x".repeat(1024 * 1024); // 1MB payload

    c.bench_function("large_payload_streaming", |b| {
        b.to_async(&rt).iter(|| async {
            let response = client
                .post(format!("http://{}/echo", proxy_listener))
                .body(large_data.clone())
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
        });
    });

    // Buffered proxy
    let proxy = ReverseProxy::new_with_options(
        "/",
        &format!("http://{}", test_addr),
        ProxyOptions {
            buffer_bodies: true,
        },
    );
    let app: Router = proxy.into();

    let buffered_proxy_listener = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(listener, app).await.unwrap();
        });

        server_ready.notified().await;
        addr
    });

    c.bench_function("large_payload_buffered", |b| {
        b.to_async(&rt).iter(|| async {
            let response = client
                .post(format!("http://{}/echo", buffered_proxy_listener))
                .body(large_data.clone())
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
        });
    });
}

fn bench_concurrent_requests(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let test_addr = rt.block_on(create_test_server());
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = rt.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(listener, app).await.unwrap();
        });

        server_ready.notified().await;
        addr
    });

    let client = reqwest::Client::new();

    c.bench_function("concurrent_requests", |b| {
        b.to_async(&rt).iter(|| async {
            let mut handles = Vec::new();
            for _ in 0..10 {
                let client = client.clone();
                let addr = proxy_listener;
                handles.push(tokio::spawn(async move {
                    let response = client
                        .get(format!("http://{}/test", addr))
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
                }));
            }
            for handle in handles {
                handle.await.unwrap();
            }
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(10))
        .sample_size(10);
    targets = bench_http1_get, bench_http2_get, bench_large_payload, bench_concurrent_requests
);
criterion_main!(benches);
