use axum::{extract::State, response::IntoResponse, routing::get, serve, Router};
use axum_reverse_proxy::ReverseProxy;
use serde_json::json;
use tokio::net::TcpListener;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

// Our app state type
#[derive(Clone)]
struct AppState {
    app_name: String,
    version: String,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .compact()
        .init();

    // Create our app state
    let state = AppState {
        app_name: "My API Gateway".to_string(),
        version: "1.0.0".to_string(),
    };

    // Create a reverse proxy that forwards requests to httpbin.org
    let proxy = ReverseProxy::new("/api", "https://httpbin.org");

    // Create our main router with app state
    let app = Router::new()
        .route("/", get(root_handler))
        .with_state(state.clone());

    // Convert proxy to router and merge
    let proxy_router: Router = proxy.into();
    let app = app.merge(proxy_router);

    // Create a TCP listener
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Server running on http://localhost:3000");
    info!("Try:");
    info!("  - GET /         -> App info");
    info!("  - GET /api/ip   -> Proxied to httpbin.org/ip");
    info!("  - GET /api/uuid -> Proxied to httpbin.org/uuid");

    // Run the server
    serve(listener, app).await.unwrap();
}

async fn root_handler(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(json!({
        "app": state.app_name,
        "version": state.version,
        "endpoints": {
            "/": "This info",
            "/api/*": "Proxied to httpbin.org"
        }
    }))
}
