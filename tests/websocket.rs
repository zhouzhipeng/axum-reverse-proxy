use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use axum_reverse_proxy::ReverseProxy;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite;
use tracing::{debug, info};
use tracing_subscriber::fmt::format::FmtSpan;

async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    // Echo server - just send back what we receive
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    if socket.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
                Message::Binary(data) => {
                    if socket.send(Message::Binary(data)).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => {
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                }
                _ => {}
            }
        } else {
            break;
        }
    }
}

async fn setup_test_server(target_prefix: &str) -> (SocketAddr, SocketAddr) {
    // Set up logging for tests
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::CLOSE)
        .with_test_writer()
        .try_init()
        .ok();

    // Create a WebSocket echo server
    let app = Router::new().route("/ws", get(websocket_handler));
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();
    info!("Upstream server listening on {}", upstream_addr);

    tokio::spawn(async move {
        axum::serve(upstream_listener, app).await.unwrap();
    });

    // Create the proxy server
    let proxy = ReverseProxy::new("/", &format!("{}{}", target_prefix, upstream_addr));
    let proxy_app: Router = proxy.into();
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    info!("Proxy server listening on {}", proxy_addr);

    tokio::spawn(async move {
        axum::serve(proxy_listener, proxy_app).await.unwrap();
    });

    (upstream_addr, proxy_addr)
}

#[tokio::test]
async fn test_websocket_upgrade() {
    let (_upstream_addr, proxy_addr) = setup_test_server("http://").await;

    // Attempt WebSocket upgrade through the proxy
    let url = format!("ws://127.0.0.1:{}/ws", proxy_addr.port());
    let _ws_client = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect");

    // If we get here, the upgrade was successful
}

#[tokio::test]
async fn test_websocket_echo() {
    let (_upstream_addr, proxy_addr) = setup_test_server("http://").await;

    // Create a WebSocket client connection through the proxy
    let url = format!("ws://127.0.0.1:{}/ws", proxy_addr.port());
    let (mut ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect");

    // Send a test message
    let test_message = "Hello, WebSocket!";
    ws_stream
        .send(tungstenite::Message::Text(test_message.into()))
        .await
        .expect("Failed to send message");

    // Receive the echo response
    if let Some(msg) = ws_stream.next().await {
        let msg = msg.expect("Failed to get message");
        assert_eq!(msg, tungstenite::Message::Text(test_message.into()));
    } else {
        panic!("Did not receive response");
    }
}

#[tokio::test]
async fn test_websocket_close() {
    let (_upstream_addr, proxy_addr) = setup_test_server("http://").await;

    // Create a WebSocket client connection through the proxy
    let url = format!("ws://127.0.0.1:{}/ws", proxy_addr.port());
    info!("Connecting to WebSocket at {}", url);
    let (mut ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect");

    // Close the connection
    ws_stream
        .close(None)
        .await
        .expect("Failed to close connection");

    // Wait for the close frame response
    while let Some(msg) = ws_stream.next().await {
        match msg {
            Ok(tungstenite::Message::Close(_)) => break,
            Ok(_) => continue,
            Err(_) => break,
        }
    }

    // Verify we don't receive any more messages
    assert!(ws_stream.next().await.is_none());
}

#[tokio::test]
async fn test_websocket_with_complex_connection_header() {
    let (_upstream_addr, proxy_addr) = setup_test_server("http://").await;

    // Create a WebSocket client with a complex Connection header
    let url = format!("ws://127.0.0.1:{}/ws", proxy_addr.port());
    let url_parsed = url::Url::parse(&url).unwrap();
    let host = url_parsed.host_str().unwrap();
    let port = url_parsed.port().unwrap_or(80);
    let host_header = if port == 80 {
        host.to_string()
    } else {
        format!("{}:{}", host, port)
    };

    let request = tokio_tungstenite::tungstenite::handshake::client::Request::builder()
        .uri(url)
        .header("Host", host_header)
        .header("Connection", "keep-alive, Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .body(())
        .unwrap();

    let (ws_stream, _) = tokio_tungstenite::connect_async(request)
        .await
        .expect("Failed to connect with complex Connection header");

    debug!("Successfully connected with complex Connection header");
    drop(ws_stream);
}

#[tokio::test]
async fn test_websocket_with_https() {
    let (_upstream_addr, proxy_addr) = setup_test_server("https://").await;

    let url = format!("ws://127.0.0.1:{}/ws", proxy_addr.port());
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect with HTTPS upstream");

    debug!("Successfully connected to HTTPS upstream");
    drop(ws_stream);
}

#[tokio::test]
async fn test_websocket_with_explicit_ws() {
    let (_upstream_addr, proxy_addr) = setup_test_server("ws://").await;

    let url = format!("ws://127.0.0.1:{}/ws", proxy_addr.port());
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect with explicit WS upstream");

    debug!("Successfully connected with explicit WS upstream");
    drop(ws_stream);
}

#[tokio::test]
async fn test_websocket_with_trailing_slash() {
    let (_upstream_addr, proxy_addr) = setup_test_server("http://").await;

    // Test with a path that includes a trailing slash
    let url = format!("ws://127.0.0.1:{}/ws/", proxy_addr.port());
    let (ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect with trailing slash");

    debug!("Successfully connected with trailing slash");
    drop(ws_stream);
}

#[tokio::test]
async fn test_websocket_binary() {
    let (_upstream_addr, proxy_addr) = setup_test_server("http://").await;

    // Create a WebSocket client connection through the proxy
    let url = format!("ws://127.0.0.1:{}/ws", proxy_addr.port());
    let (mut ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .expect("Failed to connect");

    // Send a binary message
    let test_data = vec![1, 2, 3, 4, 5];
    ws_stream
        .send(tungstenite::Message::Binary(test_data.clone()))
        .await
        .expect("Failed to send binary message");

    // Receive the echo response
    if let Some(msg) = ws_stream.next().await {
        let msg = msg.expect("Failed to get message");
        assert_eq!(msg, tungstenite::Message::Binary(test_data));
    } else {
        panic!("Did not receive response");
    }
}
