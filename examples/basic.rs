use axum::serve;
use rproxy::ReverseProxy;
use tokio::net::TcpListener;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    // Initialize tracing
    let _subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .compact()
        .init();

    // Create a reverse proxy that forwards requests to httpbin.org
    let proxy = ReverseProxy::new("https://httpbin.org");

    // Create a router from the proxy
    let app = proxy.router();

    // Create a TCP listener
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Reverse proxy server running on http://localhost:3000");

    // Run the server
    serve(listener, app).await.unwrap();
}
