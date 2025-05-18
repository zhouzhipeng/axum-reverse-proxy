use axum::body::Body;
use http::StatusCode;
use http_body_util::BodyExt;
#[cfg(feature = "tls")]
use hyper_tls::HttpsConnector;
use hyper_util::client::legacy::{
    connect::{Connect, HttpConnector},
    Client,
};
use std::convert::Infallible;
use tracing::{error, trace};

use crate::websocket;

/// A reverse proxy that forwards HTTP requests to an upstream server.
///
/// The `ReverseProxy` struct handles the forwarding of HTTP requests from a specified path
/// to a target upstream server. It manages its own HTTP client with configurable settings
/// for connection pooling, timeouts, and retries.
#[derive(Clone)]
pub struct ReverseProxy<C: Connect + Clone + Send + Sync + 'static> {
    path: String,
    target: String,
    client: Client<C, Body>,
}

#[cfg(feature = "tls")]
pub type StandardReverseProxy = ReverseProxy<HttpsConnector<HttpConnector>>;
#[cfg(not(feature = "tls"))]
pub type StandardReverseProxy = ReverseProxy<HttpConnector>;

impl StandardReverseProxy {
    /// Creates a new `ReverseProxy` instance.
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

        #[cfg(feature = "tls")]
        let connector = HttpsConnector::new_with_connector(connector);

        let client = Client::builder(hyper_util::rt::TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(32)
            .retry_canceled_requests(true)
            .set_host(true)
            .build(connector);

        Self::new_with_client(path, target, client)
    }
}

impl<C: Connect + Clone + Send + Sync + 'static> ReverseProxy<C> {
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
    /// use hyper_util::rt::TokioExecutor;
    ///
    /// let client = Client::builder(TokioExecutor::new())
    ///     .pool_idle_timeout(std::time::Duration::from_secs(120))
    ///     .build(HttpConnector::new());
    ///
    /// let proxy = ReverseProxy::new_with_client(
    ///     "/api",
    ///     "https://api.example.com",
    ///     client,
    /// );
    /// ```
    pub fn new_with_client<S>(path: S, target: S, client: Client<C, Body>) -> Self
    where
        S: Into<String>,
    {
        Self {
            path: path.into(),
            target: target.into(),
            client,
        }
    }

    /// Get the base path this proxy is configured to handle
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Get the target URL this proxy forwards requests to
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Handles the proxying of a single request to the upstream server.
    pub async fn proxy_request(
        &self,
        req: axum::http::Request<Body>,
    ) -> Result<axum::http::Response<Body>, Infallible> {
        self.handle_request(req).await
    }

    /// Core proxy logic used by the [`tower::Service`] implementation.
    async fn handle_request(
        &self,
        req: axum::http::Request<Body>,
    ) -> Result<axum::http::Response<Body>, Infallible> {
        trace!("Proxying request method={} uri={}", req.method(), req.uri());
        trace!("Original headers headers={:?}", req.headers());

        // Check if this is a WebSocket upgrade request
        if websocket::is_websocket_upgrade(req.headers()) {
            trace!("Detected WebSocket upgrade request");
            match websocket::handle_websocket(req, &self.target).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    error!("Failed to handle WebSocket upgrade: {}", e);
                    return Ok(axum::http::Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from(format!("WebSocket upgrade failed: {}", e)))
                        .unwrap());
                }
            }
        }

        let forward_req = {
            let mut builder =
                axum::http::Request::builder()
                    .method(req.method().clone())
                    .uri(self.transform_uri(
                        req.uri().path_and_query().map(|x| x.as_str()).unwrap_or(""),
                    ));

            // Forward headers
            for (key, value) in req.headers() {
                if key != "host" {
                    builder = builder.header(key, value);
                }
            }

            // Take the request body
            let (parts, body) = req.into_parts();
            drop(parts);
            builder.body(body).unwrap()
        };

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

                let (parts, body) = res.into_parts();
                let body = Body::from_stream(body.into_data_stream());

                let mut response = axum::http::Response::new(body);
                *response.status_mut() = parts.status;
                *response.version_mut() = parts.version;
                *response.headers_mut() = parts.headers;
                Ok(response)
            }
            Err(e) => {
                let error_msg = e.to_string();
                error!("Proxy error occurred err={}", error_msg);
                Ok(axum::http::Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::from(format!(
                        "Failed to connect to upstream server: {}",
                        error_msg
                    )))
                    .unwrap())
            }
        }
    }

    /// Transform an incoming request path into the target URI
    fn transform_uri(&self, path: &str) -> String {
        let target = self.target.trim_end_matches('/');
        let base_path = self.path.trim_end_matches('/');

        // Handle root path specially
        if path == "/" && !self.path.is_empty() {
            // When accessing the root of a proxy path, don't add a trailing slash
            target.to_string()
        } else if path.starts_with(&self.path) {
            let remaining = &path[base_path.len()..];
            format!("{}{}", target, remaining)
        } else {
            format!("{}{}", target, path)
        }
    }
}

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::Service;

impl<C> Service<axum::http::Request<Body>> for ReverseProxy<C>
where
    C: Connect + Clone + Send + Sync + 'static,
{
    type Response = axum::http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: axum::http::Request<Body>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move { this.handle_request(req).await })
    }
}
