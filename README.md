# axum-reverse-proxy

[![CI](https://github.com/tom-lubenow/axum-reverse-proxy/actions/workflows/ci.yml/badge.svg)](https://github.com/tom-lubenow/axum-reverse-proxy/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/axum-reverse-proxy.svg)](https://crates.io/crates/axum-reverse-proxy)
[![Documentation](https://docs.rs/axum-reverse-proxy/badge.svg)](https://docs.rs/axum-reverse-proxy)

A flexible and efficient reverse proxy implementation for [Axum](https://github.com/tokio-rs/axum) web applications. This library provides a simple way to forward HTTP requests from your Axum application to upstream servers. It is intended to be a simple implementation sitting on top of axum and hyper.

The eventual goal would be to benchmark ourselves against common reverse proxy libraries like nginx, traefik, haproxy, etc. We hope to achieve comparable (or better) performance but with significantly better developer ergonomics, using Rust code to configure the proxy instead of various configuration files with their own DSLs.

## Features

- 🛣 Path-based routing
- 🔄 Automatic retry mechanism with exponential backoff
- 📨 Header forwarding (with host header management)
- ⚙ Configurable HTTP client settings
- 🔌 Easy integration with Axum's Router
- 🧰 Custom client configuration support
- 🔒 HTTPS support
- 📋 Optional RFC9110 compliance layer
- 🔧 Full Tower middleware support

## Installation

Run `cargo add axum-reverse-proxy` to add the library to your project.

## Quick Start

Here's a simple example that forwards all requests under `/api` to `httpbin.org`:

```rust
use axum::Router;
use axum_reverse_proxy::ReverseProxy;

// Create a reverse proxy that forwards requests from /api to httpbin.org
let proxy = ReverseProxy::new("/api", "https://httpbin.org");

// Convert the proxy to a router and use it in your Axum application
let app: Router = proxy.into();
```

## Advanced Usage

### Using Tower Middleware

The proxy integrates seamlessly with Tower middleware. Common use cases include:

- Authentication and authorization
- Rate limiting
- Request validation
- Logging and tracing
- Timeouts and retries
- Caching
- Compression
- Request buffering (via tower-buffer)

Example using tower-buffer for request buffering:

```rust
use axum::Router;
use axum_reverse_proxy::ReverseProxy;
use tower::ServiceBuilder;
use tower_buffer::BufferLayer;

let proxy = ReverseProxy::new("/api", "https://api.example.com");
let app: Router = proxy.into();

// Add buffering middleware
let app = app.layer(ServiceBuilder::new().layer(BufferLayer::new(100)));
```

### Merging with Existing Router

You can merge the proxy with your existing Axum router:

```rust
use axum::{routing::get, Router, response::IntoResponse, extract::State};
use axum_reverse_proxy::ReverseProxy;

#[derive(Clone)]
struct AppState { foo: usize }

async fn root_handler(State(state): State<AppState>) -> impl IntoResponse {
    (axum::http::StatusCode::OK, format!("Hello, World! {}", state.foo))
}

let app: Router<AppState> = Router::new()
    .route("/", get(root_handler))
    .merge(ReverseProxy::new("/api", "https://httpbin.org"))
    .with_state(AppState { foo: 42 });
```

## RFC9110 Compliance

The library includes an optional RFC9110 compliance layer that implements key requirements from [RFC9110 (HTTP Semantics)](https://www.rfc-editor.org/rfc/rfc9110.html). To use it:

```rust
use axum_reverse_proxy::{ReverseProxy, Rfc9110Config, Rfc9110Layer};
use std::collections::HashSet;

// Create a config for RFC9110 compliance
let mut server_names = HashSet::new();
server_names.insert("example.com".to_string());

let config = Rfc9110Config {
    server_names: Some(server_names),  // For loop detection
    pseudonym: Some("myproxy".to_string()),  // For Via headers
    combine_via: true,  // Combine Via headers with same protocol
};

// Create a proxy with RFC9110 compliance
let proxy = ReverseProxy::new("/api", "https://api.example.com")
    .layer(Rfc9110Layer::with_config(config));
```

The RFC9110 layer provides:

- **Connection Header Processing**: Properly handles Connection headers and removes hop-by-hop headers
- **Via Header Management**: Adds and combines Via headers according to spec, with optional firewall mode
- **Max-Forwards Processing**: Handles Max-Forwards header for TRACE/OPTIONS methods
- **Loop Detection**: Detects request loops using Via headers and server names
- **End-to-end Header Preservation**: Preserves end-to-end headers while removing hop-by-hop headers

### Custom Client Configuration

For more control over the HTTP client behavior:

```rust
use axum_reverse_proxy::ReverseProxy;
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use axum::body::Body;

let mut connector = HttpConnector::new();
connector.set_nodelay(true);
connector.enforce_http(false);
connector.set_keepalive(Some(std::time::Duration::from_secs(60)));

let client = Client::builder(hyper_util::rt::TokioExecutor::new())
    .pool_idle_timeout(std::time::Duration::from_secs(60))
    .pool_max_idle_per_host(32)
    .build(connector);

let proxy = ReverseProxy::new_with_client("/api", "https://api.example.com", client);
```

## Configuration

The default configuration includes:

- 3 retry attempts with exponential backoff
- 60-second keepalive timeout
- 10-second connect timeout
- TCP nodelay enabled
- Connection pooling with 32 idle connections per host
- Automatic host header management

## Examples

Check out the [examples](examples/) directory for more usage examples:

- [Basic Proxy](examples/nested.rs) - Shows how to set up a basic reverse proxy with path-based routing

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
