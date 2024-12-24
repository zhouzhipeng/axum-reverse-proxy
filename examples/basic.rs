use rproxy::ReverseProxy;
use axum::serve;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // Create a reverse proxy that forwards requests to httpbin.org
    let proxy = ReverseProxy::new("https://httpbin.org");
    
    // Create a router from the proxy
    let app = proxy.router();
    
    // Create a TCP listener
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Reverse proxy server running on http://localhost:3000");
    
    // Run the server
    serve(listener, app).await.unwrap();
} 