use axum::{
    body::Body,
    http::{HeaderMap, Request, Response, StatusCode},
    response::IntoResponse,
    routing::any,
    Router,
};
use http_body_util::BodyExt;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use std::convert::Infallible;
use tracing::{error, trace};

pub struct ReverseProxy {
    target: String,
    client: Client<HttpConnector, Body>,
}

impl ReverseProxy {
    pub fn new(target: &str) -> Self {
        let mut connector = HttpConnector::new();
        connector.set_nodelay(true);
        connector.enforce_http(false);

        let client = Client::builder(TokioExecutor::new())
            .http2_only(true)
            .build(connector);

        Self {
            target: target.to_string(),
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
                return Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response());
            }
        };
        trace!("Request body collected body_length={}", body_bytes.len());

        // Build the new request
        let mut forward_req = Request::builder()
            .method(parts.method)
            .uri(format!(
                "{}{}",
                self.target,
                parts.uri.path_and_query().map(|x| x.as_str()).unwrap_or("")
            ))
            .body(Body::from(body_bytes))
            .unwrap();

        // Forward headers
        let mut forward_headers = HeaderMap::new();
        for (key, value) in parts.headers.iter() {
            if key != "host" {
                forward_headers.insert(key, value.clone());
            }
        }
        *forward_req.headers_mut() = forward_headers;
        trace!(
            "Forwarding headers forwarded_headers={:?}",
            forward_req.headers()
        );

        // Send the request
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
                        return Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response());
                    }
                };
                trace!("Response body collected body_length={}", body_bytes.len());

                // Build and return the response
                let mut response = Response::builder()
                    .status(parts.status)
                    .body(Body::from(body_bytes))
                    .unwrap();

                *response.headers_mut() = parts.headers;
                Ok(response)
            }
            Err(e) => {
                error!("Proxy error occurred err={}", e);
                Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response())
            }
        }
    }

    pub fn router(self) -> Router {
        Router::new().fallback(any(move |req| {
            let proxy = self.clone();
            async move { proxy.proxy_request(req).await }
        }))
    }
}

impl Clone for ReverseProxy {
    fn clone(&self) -> Self {
        Self::new(&self.target)
    }
}
