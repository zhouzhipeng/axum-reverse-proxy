use axum::{extract::Json, http::StatusCode, routing::get, Router};
use axum_reverse_proxy::BalancedProxy;
use serde_json::json;
use serde_json::Value;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_round_robin_distribution() {
    let app1 = Router::new().route("/", get(|| async { Json(json!({"server": 1})) }));
    let listener1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr1 = listener1.local_addr().unwrap();
    let server1 = tokio::spawn(async move { axum::serve(listener1, app1).await.unwrap() });

    let app2 = Router::new().route("/", get(|| async { Json(json!({"server": 2})) }));
    let listener2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr2 = listener2.local_addr().unwrap();
    let server2 = tokio::spawn(async move { axum::serve(listener2, app2).await.unwrap() });
    let proxy = BalancedProxy::new(
        String::from("/"),
        vec![format!("http://{}", addr1), format!("http://{}", addr2)],
    );
    let app: Router = proxy.into();

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_server = tokio::spawn(async move { axum::serve(proxy_listener, app).await.unwrap() });

    let client = reqwest::Client::new();
    for expected in [1, 2, 1, 2] {
        let res = client
            .get(format!("http://{}/", proxy_addr))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status().as_u16(), StatusCode::OK.as_u16());
        let body: Value = res.json().await.unwrap();
        assert_eq!(body["server"], expected);
    }

    proxy_server.abort();
    server1.abort();
    server2.abort();
}

#[tokio::test]
async fn test_balanced_proxy_no_upstreams_returns_503() {
    use axum::body::Body;
    use tower::Service;

    let mut proxy = BalancedProxy::new(String::from("/"), Vec::<String>::new());

    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/test")
        .body(Body::empty())
        .unwrap();

    let response = proxy.call(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert!(body_str.contains("No upstream services available"));
}
