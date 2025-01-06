use axum::Router;
use axum_reverse_proxy::ReverseProxy;
use std::env;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get configuration from environment variables
    let proxy_path = env::var("PROXY_PATH").unwrap_or_else(|_| "/".to_string());
    let proxy_target = env::var("PROXY_TARGET").expect("PROXY_TARGET must be set");
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let port = port.parse::<u16>().expect("PORT must be a valid number");

    // Create the proxy and convert it to a router
    let proxy = ReverseProxy::new(proxy_path, proxy_target);
    let app: Router = proxy.into();

    // Run the server
    let listener = TcpListener::bind(("0.0.0.0", port)).await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
} 