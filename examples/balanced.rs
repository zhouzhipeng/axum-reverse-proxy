use axum::{serve, Router};
use axum_reverse_proxy::BalancedProxy;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    // Forward /api to two upstream servers
    let proxy = BalancedProxy::new(
        "/api",
        vec!["https://api1.example.com", "https://api2.example.com"],
    );
    let app: Router = proxy.into();

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://localhost:3000");
    serve(listener, app).await.unwrap();
}
