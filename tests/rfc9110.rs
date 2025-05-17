use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use axum_reverse_proxy::{ReverseProxy, Rfc9110Config, Rfc9110Layer};
use http::HeaderValue;
use std::collections::HashSet;
use tower::ServiceExt;

/// Helper function to create a test app with RFC9110 middleware
fn create_test_app(config: Option<Rfc9110Config>) -> Router {
    let proxy = ReverseProxy::new("/", "http://example.com");
    let proxy_router: Router = proxy.into();
    let app = Router::new().merge(proxy_router);

    if let Some(config) = config {
        app.layer(Rfc9110Layer::with_config(config))
    } else {
        app.layer(Rfc9110Layer::new())
    }
}

#[tokio::test]
async fn test_connection_header_removal() {
    let app = create_test_app(None);

    let request = Request::builder()
        .uri("/test")
        .header("Connection", "close, x-custom-header")
        .header("x-custom-header", "value")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    assert!(!response.headers().contains_key("connection"));
    assert!(!response.headers().contains_key("x-custom-header"));
}

#[tokio::test]
async fn test_hop_by_hop_header_removal() {
    let app = create_test_app(None);

    let request = Request::builder()
        .uri("/test")
        .header("Keep-Alive", "timeout=5")
        .header("Transfer-Encoding", "chunked")
        .header("TE", "trailers")
        .header("Upgrade", "websocket")
        .header("Proxy-Connection", "keep-alive")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // All hop-by-hop headers should be removed
    assert!(!response.headers().contains_key("keep-alive"));
    assert!(!response.headers().contains_key("transfer-encoding"));
    assert!(!response.headers().contains_key("te"));
    assert!(!response.headers().contains_key("upgrade"));
    assert!(!response.headers().contains_key("proxy-connection"));
}

#[tokio::test]
async fn test_max_forwards_trace() {
    let app = create_test_app(None);

    // Test Max-Forwards = 0
    let request = Request::builder()
        .method(Method::TRACE)
        .uri("/test")
        .header("Max-Forwards", "0")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should not forward request when Max-Forwards is 0
    assert_eq!(response.status(), StatusCode::OK);

    // Test Max-Forwards > 0
    let request = Request::builder()
        .method(Method::TRACE)
        .uri("/test")
        .header("Max-Forwards", "5")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should decrement Max-Forwards
    assert_eq!(
        response.headers().get("max-forwards").unwrap(),
        HeaderValue::from_static("4")
    );
}

#[tokio::test]
async fn test_max_forwards_options() {
    let app = create_test_app(None);

    // Test Max-Forwards with OPTIONS method
    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/test")
        .header("Max-Forwards", "3")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    assert_eq!(
        response.headers().get("max-forwards").unwrap(),
        HeaderValue::from_static("2")
    );
}

#[tokio::test]
async fn test_max_forwards_ignored_for_other_methods() {
    let app = create_test_app(None);

    // Max-Forwards should be ignored for other methods
    let request = Request::builder()
        .method(Method::GET)
        .uri("/test")
        .header("Max-Forwards", "5")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Max-Forwards should be unchanged
    assert_eq!(
        response.headers().get("max-forwards").unwrap(),
        HeaderValue::from_static("5")
    );
}

#[tokio::test]
async fn test_via_header_basic() {
    let app = create_test_app(None);

    let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should add Via header with default pseudonym
    assert!(response.headers().contains_key("via"));
    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("1.1 proxy")
    );
}

#[tokio::test]
async fn test_via_header_custom_pseudonym() {
    let config = Rfc9110Config {
        pseudonym: Some("test-proxy".to_string()),
        ..Default::default()
    };
    let app = create_test_app(Some(config));

    let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("1.1 test-proxy")
    );
}

#[tokio::test]
async fn test_via_header_combining() {
    let config = Rfc9110Config {
        pseudonym: Some("test-proxy".to_string()),
        combine_via: true,
        ..Default::default()
    };
    let app = create_test_app(Some(config));

    let request = Request::builder()
        .uri("/test")
        .header("Via", "1.1 proxy1, 1.1 proxy2")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should combine Via headers with same protocol version
    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("1.1 test-proxy")
    );
}

#[tokio::test]
async fn test_loop_detection() {
    let mut server_names = HashSet::new();
    server_names.insert("example.com".to_string());

    let config = Rfc9110Config {
        server_names: Some(server_names),
        ..Default::default()
    };
    let app = create_test_app(Some(config));

    // Test request to self
    let request = Request::builder()
        .uri("http://example.com/test")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should detect and prevent loop
    assert_eq!(response.status(), StatusCode::LOOP_DETECTED);
}

#[tokio::test]
async fn test_unknown_headers_forwarded() {
    let app = create_test_app(None);

    let request = Request::builder()
        .uri("/test")
        .header("X-Custom-Unknown", "value")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Unknown headers should be forwarded
    assert_eq!(
        response.headers().get("x-custom-unknown").unwrap(),
        HeaderValue::from_static("value")
    );
}

#[tokio::test]
async fn test_end_to_end_headers_preserved() {
    let app = create_test_app(None);

    let request = Request::builder()
        .uri("/test")
        .header("Cache-Control", "no-cache")
        .header("Authorization", "Bearer token")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // End-to-end headers should be preserved
    assert_eq!(
        response.headers().get("cache-control").unwrap(),
        HeaderValue::from_static("no-cache")
    );
    assert_eq!(
        response.headers().get("authorization").unwrap(),
        HeaderValue::from_static("Bearer token")
    );
}

#[tokio::test]
async fn test_connection_header_case_insensitive() {
    let app = create_test_app(None);

    let request = Request::builder()
        .uri("/test")
        .header("CONNECTION", "Close, X-Custom-Header")
        .header("X-CUSTOM-HEADER", "value")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Headers should be removed regardless of case
    assert!(!response.headers().contains_key("connection"));
    assert!(!response.headers().contains_key("x-custom-header"));
}

#[tokio::test]
async fn test_via_header_protocol_version_detection() {
    let app = create_test_app(None);

    // Test HTTP/1.0 request
    let request = Request::builder()
        .uri("/test")
        .version(http::Version::HTTP_10)
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("1.0 proxy")
    );

    // Test HTTP/1.1 request
    let request = Request::builder()
        .uri("/test")
        .version(http::Version::HTTP_11)
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("1.1 proxy")
    );

    // Test HTTP/2 request
    let request = Request::builder()
        .uri("/test")
        .version(http::Version::HTTP_2)
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("2.0 proxy")
    );
}

#[tokio::test]
async fn test_via_header_multiple_protocols() {
    let config = Rfc9110Config {
        pseudonym: Some("test-proxy".to_string()),
        combine_via: true,
        ..Default::default()
    };
    let app = create_test_app(Some(config));

    let request = Request::builder()
        .uri("/test")
        .header("Via", "1.0 old-proxy, 1.1 modern-proxy, 2.0 new-proxy")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should not combine headers with different protocol versions
    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("1.0 old-proxy, 1.1 modern-proxy, 2.0 new-proxy, 1.1 test-proxy")
    );
}

#[tokio::test]
async fn test_via_header_with_comments() {
    let app = create_test_app(None);

    let request = Request::builder()
        .uri("/test")
        .header(
            "Via",
            "1.1 proxy1 (Proxy Software 1.0), 1.1 proxy2 (Gateway)",
        )
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Comments should be preserved
    assert!(response
        .headers()
        .get("via")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("("));
}

#[tokio::test]
async fn test_via_header_with_ports() {
    let config = Rfc9110Config {
        pseudonym: Some("test-proxy:8080".to_string()),
        ..Default::default()
    };
    let app = create_test_app(Some(config));

    let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Port should be included in Via header
    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("1.1 test-proxy:8080")
    );
}

#[tokio::test]
async fn test_max_forwards_zero_trace_response() {
    let app = create_test_app(None);

    let request = Request::builder()
        .method(Method::TRACE)
        .uri("/test")
        .header("Max-Forwards", "0")
        .body(Body::empty())
        .unwrap();
    // Capture a debug representation of the original request before it's moved
    let request_debug = format!("{:?}", &request);

    let response = app.clone().oneshot(request).await.unwrap();

    // Should return 200 OK with the request as the body
    assert_eq!(response.status(), StatusCode::OK);
    // Read the body and ensure it contains the original request text
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = std::str::from_utf8(&body_bytes).unwrap();
    assert!(body_str.contains(&request_debug));
}

#[tokio::test]
async fn test_max_forwards_zero_options_response() {
    let app = create_test_app(None);

    let request = Request::builder()
        .method(Method::OPTIONS)
        .uri("/test")
        .header("Max-Forwards", "0")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should return 200 OK with allowed methods
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("allow"));
}

#[tokio::test]
async fn test_loop_detection_with_aliases() {
    let mut server_names = HashSet::new();
    server_names.insert("example.com".to_string());
    server_names.insert("www.example.com".to_string());
    server_names.insert("example.net".to_string()); // Alias

    let config = Rfc9110Config {
        server_names: Some(server_names),
        ..Default::default()
    };
    let app = create_test_app(Some(config));

    // Test request to alias
    let request = Request::builder()
        .uri("http://example.net/test")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should detect loop through alias
    assert_eq!(response.status(), StatusCode::LOOP_DETECTED);
}

#[tokio::test]
async fn test_loop_detection_with_ip() {
    let mut server_names = HashSet::new();
    server_names.insert("127.0.0.1".to_string());
    server_names.insert("localhost".to_string());

    let config = Rfc9110Config {
        server_names: Some(server_names),
        ..Default::default()
    };
    let app = create_test_app(Some(config));

    // Test request to IP
    let request = Request::builder()
        .uri("http://127.0.0.1/test")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should detect loop through IP
    assert_eq!(response.status(), StatusCode::LOOP_DETECTED);
}

#[tokio::test]
async fn test_connection_header_with_end_to_end_field() {
    let app = create_test_app(None);

    let request = Request::builder()
        .uri("/test")
        .header("Connection", "Cache-Control")
        .header("Cache-Control", "no-cache")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Cache-Control should be preserved even if listed in Connection
    assert!(response.headers().contains_key("cache-control"));
    assert!(!response.headers().contains_key("connection"));
}

#[tokio::test]
async fn test_via_header_firewall_mode() {
    let config = Rfc9110Config {
        pseudonym: Some("firewall".to_string()),
        ..Default::default()
    };
    let app = create_test_app(Some(config));

    let request = Request::builder()
        .uri("/test")
        .header("Via", "1.1 internal-proxy.local")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Internal proxy name should be replaced with pseudonym
    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("1.1 firewall")
    );
}

#[tokio::test]
async fn test_via_header_combining_same_protocol() {
    let config = Rfc9110Config {
        pseudonym: Some("merged-proxy".to_string()),
        combine_via: true,
        ..Default::default()
    };
    let app = create_test_app(Some(config));

    let request = Request::builder()
        .uri("/test")
        .header(
            "Via",
            "1.1 proxy1 (comment1), 1.1 proxy2 (comment2), 1.1 proxy3",
        )
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should combine all 1.1 proxies
    assert_eq!(
        response.headers().get("via").unwrap(),
        HeaderValue::from_static("1.1 merged-proxy")
    );
}

#[tokio::test]
async fn test_unknown_protocol_elements() {
    let app = create_test_app(None);

    let request = Request::builder()
        .uri("/test")
        .method("CUSTOM_METHOD") // Non-standard method
        .header("X-Custom-Protocol", "value") // Non-standard header
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Unknown elements should be forwarded
    assert!(response.headers().contains_key("x-custom-protocol"));
}

#[tokio::test]
async fn test_connection_specific_field_without_connection_header() {
    let app = create_test_app(None);

    let request = Request::builder()
        .uri("/test")
        .header("Proxy-Connection", "keep-alive") // Connection-specific without Connection header
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();

    // Should be removed even without Connection header
    assert!(!response.headers().contains_key("proxy-connection"));
}
