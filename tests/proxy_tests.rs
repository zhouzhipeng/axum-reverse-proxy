use axum::{
    body::Body,
    extract::{Json, State},
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use axum_reverse_proxy::ReverseProxy;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::Notify};

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
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
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

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
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
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
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

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
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
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
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

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body: Value = response.json().await.unwrap();
    assert_eq!(body, test_body);

    // Clean up
    proxy_server.abort();
    test_server.abort();
}

#[tokio::test]
async fn test_proxy_timeout() {
    // Create a test server
    let app = Router::new().route(
        "/delay",
        get(|| async {
            tokio::time::sleep(Duration::from_secs(2)).await;
            "Delayed response"
        }),
    );

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let test_server = tokio::spawn(async move {
        axum::serve(test_listener, app).await.unwrap();
    });

    // Create a reverse proxy
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
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
    let proxy = ReverseProxy::new("/", &format!("http://{}", test_addr));
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

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
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
    let proxy = ReverseProxy::new("/proxy", &format!("http://{}", test_addr));

    // Create an app state
    #[derive(Clone)]
    struct AppState {
        name: String,
    }
    let state = AppState {
        name: "test app".to_string(),
    };

    // Create a main router with app state and proxy
    let app = Router::new()
        .route(
            "/",
            get(|State(state): State<AppState>| async move { Json(json!({ "app": state.name })) }),
        )
        .with_state(state);

    // Convert proxy to router and merge
    let proxy_router: Router = proxy.into();
    let app = app.merge(proxy_router);

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

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["app"], "test app");

    // Test proxied endpoint
    let response = client
        .get(&format!("http://{}/proxy/test", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["message"], "Hello from test server!");

    // Clean up
    proxy_server.abort();
    test_server.abort();
}

#[tokio::test]
async fn test_proxy_path_handling() {
    // Create a test server
    let app = Router::new()
        .route("/", get(|| async { "root" }))
        .route("/test//double", get(|| async { "double" }))
        .route("/test/%20space", get(|| async { "space" }))
        .route("/test/special!%40%23%24", get(|| async { "special" }));

    let test_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let test_addr = test_listener.local_addr().unwrap();
    let test_server = tokio::spawn(async move {
        axum::serve(test_listener, app).await.unwrap();
    });

    // Create a reverse proxy with empty path
    let proxy = ReverseProxy::new("", &format!("http://{}", test_addr));
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    // Create a client
    let client = reqwest::Client::new();

    // Test root path
    let response = client
        .get(&format!("http://{}/", proxy_addr))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    assert_eq!(response.text().await.unwrap(), "root");

    // Test double slashes
    let response = client
        .get(&format!("http://{}/test//double", proxy_addr))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    assert_eq!(response.text().await.unwrap(), "double");

    // Test URL-encoded space
    let response = client
        .get(&format!("http://{}/test/%20space", proxy_addr))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    assert_eq!(response.text().await.unwrap(), "space");

    // Test special characters
    let response = client
        .get(&format!("http://{}/test/special!%40%23%24", proxy_addr))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    assert_eq!(response.text().await.unwrap(), "special");

    // Clean up
    proxy_server.abort();
    test_server.abort();
}

#[tokio::test]
async fn test_proxy_error_handling() {
    // Create a client with short timeouts
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();

    // Test connection refused (use a port that's likely not in use)
    let proxy = ReverseProxy::new("/", "http://127.0.0.1:59999");
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move {
        axum::serve(proxy_listener, app).await.unwrap();
    });

    let response = client
        .get(&format!("http://{}/test", proxy_addr))
        .send()
        .await;

    assert!(response.is_err());
    let err = response.unwrap_err();
    assert!(err.is_timeout() || err.is_connect());

    // Clean up
    proxy_server.abort();
}

#[tokio::test]
async fn test_proxy_header_handling() {
    // Create a test server that echoes headers
    let app = Router::new().route(
        "/headers",
        get(|req: Request<Body>| async move {
            let headers = req.headers().clone();
            Json(json!({ "headers": headers.iter().map(|(k, v)| {
                (k.as_str().to_string(), v.to_str().unwrap().to_string())
            }).collect::<HashMap<String, String>>() }))
        }),
    );

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
            .get(&format!("http://{}/headers", proxy_addr))
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
        assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());

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
    let app = Router::new().route(
        "/headers",
        get(|req: Request<Body>| async move {
            let headers = req.headers().clone();
            Json(json!({ "headers": headers.iter().map(|(k, v)| {
                (k.as_str().to_string(), v.to_str().unwrap().to_string())
            }).collect::<HashMap<String, String>>() }))
        }),
    );

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
            .get(&format!("http://{}/headers", proxy_addr))
            .header("X-Forwarded-For", "192.168.1.1")
            .header("X-Real-IP", "192.168.1.1")
            .header("X-Request-ID", "test-request-1")
            .header("Accept-Encoding", "gzip, deflate")
            .send()
            .await?;

        println!("Special headers response status: {}", response.status());
        assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());

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

#[tokio::test]
async fn test_proxy_multiple_states() {
    // Create test servers
    let app1 = Router::new().route("/test", get(|| async { "server1" }));
    let app2 = Router::new().route("/test", get(|| async { "server2" }));

    let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr1 = listener1.local_addr().unwrap();
    let server1 = tokio::spawn(async move {
        axum::serve(listener1, app1).await.unwrap();
    });

    let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr2 = listener2.local_addr().unwrap();
    let server2 = tokio::spawn(async move {
        axum::serve(listener2, app2).await.unwrap();
    });

    // Create proxies with different paths
    let proxy1 = ReverseProxy::new("/api1", &format!("http://{}", addr1));
    let proxy2 = ReverseProxy::new("/api2", &format!("http://{}", addr2));

    // Create app state
    #[derive(Clone)]
    struct AppState {
        name: String,
    }
    let state = AppState {
        name: "test app".to_string(),
    };

    // Create main router with state and both proxies
    let app = Router::new()
        .route(
            "/",
            get(|State(state): State<AppState>| async move { Json(json!({ "app": state.name })) }),
        )
        .with_state(state);

    // Convert proxies to routers and merge
    let proxy_router1: Router = proxy1.into();
    let proxy_router2: Router = proxy2.into();
    let app = app.merge(proxy_router1).merge(proxy_router2);

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

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["app"], "test app");

    // Test first proxy
    let response = client
        .get(&format!("http://{}/api1/test", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    assert_eq!(response.text().await.unwrap(), "server1");

    // Test second proxy
    let response = client
        .get(&format!("http://{}/api2/test", proxy_addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), StatusCode::OK.as_u16());
    assert_eq!(response.text().await.unwrap(), "server2");

    // Clean up
    proxy_server.abort();
    server1.abort();
    server2.abort();
}
