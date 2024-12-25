//! A flexible and efficient reverse proxy implementation for Axum web applications.
//!
//! This crate provides a reverse proxy that can be easily integrated into Axum applications,
//! allowing for seamless forwarding of HTTP requests to upstream servers. It supports:
//!
//! - Path-based routing
//! - Automatic retry mechanism
//! - Header forwarding
//! - Configurable HTTP client settings
//! - Easy integration with Axum's Router
//!
//! # Example
//!
//! ```rust
//! use axum::Router;
//! use axum_reverse_proxy::ReverseProxy;
//!
//! // Create a reverse proxy that forwards requests from /api to httpbin.org
//! let proxy = ReverseProxy::new("/api", "https://httpbin.org");
//!
//! // Convert the proxy to a router and use it in your Axum application
//! let app: Router = proxy.into();
//! ```
//!
//!  You can also merge the proxy with an existing router, compatible with arbitrary state:
//!
//! ```rust
//! use axum::{routing::get, Router, response::IntoResponse, extract::State};
//! use axum_reverse_proxy::ReverseProxy;
//!
//! #[derive(Clone)]
//! struct AppState { foo: usize }
//!
//! async fn root_handler(State(state): State<AppState>) -> impl IntoResponse {
//!     (axum::http::StatusCode::OK, format!("Hello, World! {}", state.foo))
//! }
//!
//! let app: Router<AppState> = Router::new()
//!     .route("/", get(root_handler))
//!     .merge(ReverseProxy::new("/api", "https://httpbin.org"))
//!     .with_state(AppState { foo: 42 });
//! ```

use axum::{body::Body, extract::State, http::Request, response::Response, Router};
use http_body_util::BodyExt;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use std::{convert::Infallible, sync::Arc};
use tracing::{error, trace};

/// A reverse proxy that forwards HTTP requests to an upstream server.
///
/// The `ReverseProxy` struct handles the forwarding of HTTP requests from a specified path
/// to a target upstream server. It manages its own HTTP client with configurable settings
/// for connection pooling, timeouts, and retries.
#[derive(Clone)]
pub struct ReverseProxy {
    path: String,
    target: String,
    client: Client<HttpConnector, Body>,
}

impl ReverseProxy {
    /// Creates a new `ReverseProxy` instance with default client settings.
    ///
    /// # Arguments
    ///
    /// * `path` - The base path to match incoming requests against (e.g., "/api")
    /// * `target` - The upstream server URL to forward requests to (e.g., "https://api.example.com")
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum_reverse_proxy::ReverseProxy;
    ///
    /// let proxy = ReverseProxy::new("/api", "https://api.example.com");
    /// ```
    pub fn new<S>(path: S, target: S) -> Self
    where
        S: Into<String>,
    {
        let mut connector = HttpConnector::new();
        connector.set_nodelay(true);
        connector.enforce_http(false);
        connector.set_keepalive(Some(std::time::Duration::from_secs(60)));
        connector.set_connect_timeout(Some(std::time::Duration::from_secs(10)));
        connector.set_reuse_address(true);

        let client = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(32)
            .retry_canceled_requests(true)
            .set_host(true)
            .build(connector);

        Self::new_with_client(path, target, client)
    }

    /// Creates a new `ReverseProxy` instance with a custom HTTP client.
    ///
    /// This method allows for more fine-grained control over the proxy behavior by accepting
    /// a pre-configured HTTP client.
    ///
    /// # Arguments
    ///
    /// * `path` - The base path to match incoming requests against
    /// * `target` - The upstream server URL to forward requests to
    /// * `client` - A custom-configured HTTP client
    ///
    /// # Example
    ///
    /// ```rust
    /// use axum_reverse_proxy::ReverseProxy;
    /// use hyper_util::client::legacy::{Client, connect::HttpConnector};
    /// use axum::body::Body;
    ///
    /// let client = Client::builder(hyper_util::rt::TokioExecutor::new())
    ///     .build(HttpConnector::new());
    ///
    /// let proxy = ReverseProxy::new_with_client("/api", "https://api.example.com", client);
    /// ```
    pub fn new_with_client<S>(path: S, target: S, client: Client<HttpConnector, Body>) -> Self
    where
        S: Into<String>,
    {
        Self {
            path: path.into(),
            target: target.into(),
            client,
        }
    }

    /// Handles the proxying of a single request to the upstream server.
    ///
    /// This method:
    /// 1. Collects the request body
    /// 2. Forwards all headers (except 'host')
    /// 3. Implements a retry mechanism with exponential backoff
    /// 4. Handles response streaming
    ///
    /// Returns a `Response` wrapped in `Result` with `Infallible` error type, ensuring
    /// that this function always returns a response, even in error cases.
    async fn proxy_request(&self, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        trace!("Proxying request method={} uri={}", req.method(), req.uri());
        trace!("Original headers headers={:?}", req.headers());

        // Collect the request body
        let (parts, body) = req.into_parts();
        let body_bytes = match body.collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => {
                error!("Failed to read request body: {}", e);
                return Ok(Response::builder().status(500).body(Body::empty()).unwrap());
            }
        };
        trace!("Request body collected body_length={}", body_bytes.len());

        // Build the new request with retries
        let mut retries = 3;
        let mut error_msg;

        loop {
            // Create a new request for each attempt
            let mut builder = Request::builder().method(parts.method.clone()).uri(format!(
                "{}{}",
                self.target,
                parts.uri.path_and_query().map(|x| x.as_str()).unwrap_or("")
            ));

            // Forward headers
            for (key, value) in parts.headers.iter() {
                if key != "host" {
                    builder = builder.header(key, value);
                }
            }

            let forward_req = builder.body(Body::from(body_bytes.clone())).unwrap();

            trace!(
                "Forwarding headers forwarded_headers={:?}",
                forward_req.headers()
            );

            match self.client.request(forward_req).await {
                Ok(res) => {
                    trace!(
                        "Received response status={} headers={:?} version={:?}",
                        res.status(),
                        res.headers(),
                        res.version()
                    );

                    // Convert the response body
                    let (parts, body) = res.into_parts();
                    let body_bytes = match body.collect().await {
                        Ok(collected) => collected.to_bytes(),
                        Err(e) => {
                            error!("Failed to read response body: {}", e);
                            return Ok(Response::builder()
                                .status(500)
                                .body(Body::empty())
                                .unwrap());
                        }
                    };
                    trace!("Response body collected body_length={}", body_bytes.len());

                    // Build and return the response
                    let mut response = Response::builder()
                        .status(parts.status)
                        .body(Body::from(body_bytes))
                        .unwrap();

                    *response.headers_mut() = parts.headers;
                    return Ok(response);
                }
                Err(e) => {
                    error_msg = e.to_string();
                    retries -= 1;
                    if retries == 0 {
                        error!("Proxy error occurred after all retries err={}", error_msg);
                        return Ok(Response::builder()
                            .status(502)
                            .body(Body::from(format!(
                                "Failed to connect to upstream server: {}",
                                error_msg
                            )))
                            .unwrap());
                    }
                    error!(
                        "Proxy error occurred, retrying ({} left) err={}",
                        retries, error_msg
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
    }
}

/// Enables conversion from a `ReverseProxy` into an Axum `Router`.
///
/// This implementation allows the reverse proxy to be easily integrated into an Axum
/// application. It handles:
///
/// - Path-based routing using the configured base path
/// - State management using `Arc` for thread-safety
/// - Fallback handling for all HTTP methods
///
/// # Example
///
/// ```rust
/// use axum::Router;
/// use axum_reverse_proxy::ReverseProxy;
///
/// let proxy = ReverseProxy::new("/api", "https://api.example.com");
/// let app: Router = proxy.into();
/// ```
impl<S> From<ReverseProxy> for Router<S>
where
    S: Send + Sync + Clone + 'static,
{
    fn from(proxy: ReverseProxy) -> Self {
        let path = proxy.path.clone();
        let proxy_router = Router::new()
            .fallback(|State(proxy): State<Arc<ReverseProxy>>, req| async move {
                proxy.proxy_request(req).await
            })
            .with_state(Arc::new(proxy));

        if ["", "/"].contains(&path.as_str()) {
            proxy_router
        } else {
            Router::new().nest(&path, proxy_router)
        }
    }
}
