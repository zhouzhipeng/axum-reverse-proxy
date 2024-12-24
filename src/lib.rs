use axum::{body::Body, extract::State, http::Request, response::Response, Router};
use http_body_util::BodyExt;
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use std::{convert::Infallible, sync::Arc};
use tracing::{error, trace};

#[derive(Clone)]
pub struct ReverseProxy {
    path: String,
    target: String,
    client: Arc<Client<HttpConnector, Body>>,
}

impl ReverseProxy {
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

        let client = Arc::new(
            Client::builder(TokioExecutor::new())
                .pool_idle_timeout(std::time::Duration::from_secs(60))
                .pool_max_idle_per_host(32)
                .retry_canceled_requests(true)
                .set_host(true)
                .build(connector),
        );

        Self {
            path: path.into(),
            target: target.into(),
            client,
        }
    }

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
