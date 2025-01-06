use axum::{extract::Query, routing::get, Router};
use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;

async fn echo(Query(params): Query<HashMap<String, String>>) -> String {
    if let Some(size) = params.get("size") {
        if let Ok(kb) = size.parse::<usize>() {
            return "x".repeat(kb * 1024);
        }
    }
    "Hello from backend!".to_string()
}

async fn slow() -> &'static str {
    sleep(Duration::from_secs(1)).await;
    "Slow response"
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(echo))
        .route("/slow", get(slow));

    let addr = "0.0.0.0:8080";
    println!("Backend listening on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
