use axum::{Router, serve};
use axum_reverse_proxy::ReverseProxy;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // Create a reverse proxy that forwards requests to httpbin.org
    let proxy = ReverseProxy::new("/", "https://httpbin.org");
    let app: Router = proxy.into();

    // Create a TCP listener
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://localhost:3000");

    // Run the server
    serve(listener, app).await.unwrap();
}
