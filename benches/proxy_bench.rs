#![allow(unused_must_use)]

use axum::{
    routing::{get, post},
    Router,
};
use axum_reverse_proxy::ReverseProxy;
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

async fn bench_http1_get(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let test_addr = create_test_server().await;
        let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
        let app: Router = proxy.into();

        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(proxy_listener, app).await.unwrap();
        });

        server_ready.notified().await;

        let client = reqwest::Client::new();

        c.bench_function("http1_get", |b| {
            b.iter(|| {
                rt.block_on(async {
                    let response = client
                        .get(format!("http://{}/test", proxy_addr))
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
                });
            });
        });
    });
}

async fn bench_http2_get(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let test_addr = create_test_server().await;
        let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
        let app: Router = proxy.into();

        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(proxy_listener, app).await.unwrap();
        });

        server_ready.notified().await;

        let client = reqwest::Client::builder()
            .http2_prior_knowledge()
            .build()
            .unwrap();

        c.bench_function("http2_get", |b| {
            b.iter(|| {
                rt.block_on(async {
                    let response = client
                        .get(format!("http://{}/test", proxy_addr))
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
                });
            });
        });
    });
}

async fn bench_large_payload(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let test_addr = create_test_server().await;
        let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
        let app: Router = proxy.into();

        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(proxy_listener, app).await.unwrap();
        });

        server_ready.notified().await;

        let client = reqwest::Client::new();
        let large_data = "x".repeat(1024 * 1024); // 1MB payload

        c.bench_function("large_payload", |b| {
            b.iter(|| {
                rt.block_on(async {
                    let response = client
                        .post(format!("http://{}/echo", proxy_addr))
                        .body(large_data.clone())
                        .send()
                        .await
                        .unwrap();
                    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
                });
            });
        });
    });
}

async fn bench_concurrent_requests(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let test_addr = create_test_server().await;
        let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
        let app: Router = proxy.into();

        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();

        let server_ready = Arc::new(Notify::new());
        let server_ready_clone = server_ready.clone();

        tokio::spawn(async move {
            server_ready_clone.notify_one();
            axum::serve(proxy_listener, app).await.unwrap();
        });

        server_ready.notified().await;

        let client = reqwest::Client::new();

        c.bench_function("concurrent_requests", |b| {
            b.iter(|| {
                rt.block_on(async {
                    let mut handles = Vec::new();
                    for _ in 0..10 {
                        let client = client.clone();
                        handles.push(tokio::spawn(async move {
                            let response = client
                                .get(format!("http://{}/test", proxy_addr))
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
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(10));
    targets = bench_http1_get, bench_http2_get, bench_large_payload, bench_concurrent_requests
);
criterion_main!(benches);
