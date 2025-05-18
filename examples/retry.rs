use axum::{serve, Router};
use axum_reverse_proxy::{RetryLayer, ReverseProxy};
use tokio::net::TcpListener;
use tower::ServiceBuilder;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Forward all requests under /api to httpbin
    let proxy = ReverseProxy::new("/api", "https://httpbin.org");

    // Enable retries
    let app: Router = proxy.into();
    let app = app.layer(ServiceBuilder::new().layer(RetryLayer::new(3)));

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://127.0.0.1:3000");
    serve(listener, app).await.unwrap();
}
