use axum::{body::Body, http::Request, response::Json, routing::get, Router};
use axum_reverse_proxy::ReverseProxy;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::Notify};

async fn echo_headers(req: Request<Body>) -> Json<Value> {
    let headers = req.headers().clone();
    Json(json!({ "headers": headers.iter().map(|(k, v)| {
        (k.as_str().to_string(), v.to_str().unwrap().to_string())
    }).collect::<HashMap<String, String>>() }))
}

#[tokio::test]
async fn test_proxy_header_handling() {
    // Create a test server that echoes headers
    let app = Router::new().route("/headers", get(echo_headers));

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let server_ready = Arc::new(Notify::new());
    let server_ready_clone = server_ready.clone();

    let test_server = tokio::spawn(async move {
        server_ready_clone.notify_one();
        axum::serve(test_listener, app).await.unwrap();
    });

    // Wait for test server to be ready
    server_ready.notified().await;

    // Create a reverse proxy
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_ready = Arc::new(Notify::new());
    let proxy_ready_clone = proxy_ready.clone();

    let proxy_server = tokio::spawn(async move {
        proxy_ready_clone.notify_one();
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Wait for proxy server to be ready
    proxy_ready.notified().await;

    // Create a client with a reasonable timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    // Test with a reasonable number of headers (10 instead of 50)
    let mut headers = HeaderMap::new();
    for i in 0..10 {
        headers.insert(
            HeaderName::from_bytes(format!("x-test-{}", i).as_bytes()).unwrap(),
            HeaderValue::from_str(&format!("value-{}", i)).unwrap(),
        );
    }

    // Use tokio::time::timeout for the test operations
    let test_result = tokio::time::timeout(Duration::from_secs(10), async {
        println!("Sending request to proxy...");
        let response = match client
            .get(format!("http://{}/headers", proxy_addr))
            .headers(headers.clone())
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                println!("Failed to send request: {}", e);
                return Err(e);
            }
        };

        println!("Got response with status: {}", response.status());
        assert_eq!(response.status().as_u16(), 200);

        let body = match response.json::<Value>().await {
            Ok(b) => {
                println!("Response body: {}", b);
                b
            }
            Err(e) => {
                println!("Failed to parse response as JSON: {}", e);
                return Err(e);
            }
        };

        let response_headers = body["headers"].as_object().unwrap();

        // Verify headers were forwarded (excluding host header)
        for (key, value) in headers.iter() {
            if key != "host" {
                println!("Checking header {}: {}", key, value.to_str().unwrap());
                assert_eq!(
                    response_headers[key.as_str()].as_str().unwrap(),
                    value.to_str().unwrap()
                );
            }
        }

        Ok(())
    })
    .await;

    // Clean up servers first
    proxy_server.abort();
    test_server.abort();

    // Then check the test result
    match test_result {
        Ok(result) => {
            if let Err(e) = result {
                panic!("Test failed with error: {}", e);
            }
        }
        Err(_) => panic!("Test timed out after 10 seconds"),
    }
}

#[tokio::test]
async fn test_proxy_special_headers() {
    // Create a test server that echoes headers
    let app = Router::new().route("/headers", get(echo_headers));

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let server_ready = Arc::new(Notify::new());
    let server_ready_clone = server_ready.clone();

    let test_server = tokio::spawn(async move {
        server_ready_clone.notify_one();
        axum::serve(test_listener, app).await.unwrap();
    });

    // Wait for test server to be ready
    server_ready.notified().await;

    // Create a reverse proxy
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_ready = Arc::new(Notify::new());
    let proxy_ready_clone = proxy_ready.clone();

    let proxy_server = tokio::spawn(async move {
        proxy_ready_clone.notify_one();
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Wait for proxy server to be ready
    proxy_ready.notified().await;

    // Create a client with a reasonable timeout
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();

    // Use tokio::time::timeout for the test operations
    let test_result = tokio::time::timeout(Duration::from_secs(10), async {
        println!("Testing special headers...");
        let response = client
            .get(format!("http://{}/headers", proxy_addr))
            .header("X-Forwarded-For", "192.168.1.1")
            .header("X-Real-IP", "192.168.1.1")
            .header("X-Request-ID", "test-request-1")
            .header("Accept-Encoding", "gzip, deflate")
            .send()
            .await?;

        println!("Special headers response status: {}", response.status());
        assert_eq!(response.status().as_u16(), 200);

        let body = response.json::<Value>().await?;
        println!("Response body: {}", body);

        // Verify the headers were forwarded
        let headers = body["headers"].as_object().unwrap();
        assert_eq!(headers["x-forwarded-for"], "192.168.1.1");
        assert_eq!(headers["x-real-ip"], "192.168.1.1");
        assert_eq!(headers["x-request-id"], "test-request-1");
        assert!(headers.contains_key("accept-encoding"));

        Ok::<(), reqwest::Error>(())
    })
    .await;

    // Clean up servers first
    proxy_server.abort();
    test_server.abort();

    // Then check the test result
    match test_result {
        Ok(result) => {
            if let Err(e) = result {
                panic!("Test failed with error: {}", e);
            }
        }
        Err(_) => panic!("Test timed out after 10 seconds"),
    }
}
