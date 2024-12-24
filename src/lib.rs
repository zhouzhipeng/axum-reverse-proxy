use axum::{
    Router,
    routing::any,
    http::{Request, Uri},
    response::Response,
    body::Body,
};
use bytes::Bytes;
use hyper_util::{
    client::legacy::Client,
    rt::TokioExecutor,
    client::legacy::connect::HttpConnector,
};
use http_body_util::{BodyExt, Full};
use tracing::{trace, error, instrument};

pub struct ReverseProxy {
    target_url: Uri,
    client: Client<HttpConnector, Full<Bytes>>,
}

impl ReverseProxy {
    #[instrument]
    pub fn new(target_url: &str) -> Self {
        trace!(target_url, "Creating new reverse proxy");
        let target_url = if target_url.ends_with('/') {
            target_url.to_string()
        } else {
            format!("{}/", target_url)
        };

        Self {
            target_url: target_url.parse().expect("Invalid target URL"),
            client: Client::builder(TokioExecutor::new()).build(HttpConnector::new()),
        }
    }

    #[instrument(skip(self, req))]
    pub async fn proxy_request(
        &self,
        req: Request<Body>,
    ) -> Result<Response, Box<dyn std::error::Error>> {
        // Extract parts we need before consuming the request
        let method = req.method().clone();
        let headers = req.headers().clone();
        let path_and_query = req.uri().path_and_query()
            .map(|x| x.as_str())
            .unwrap_or("");
            
        // Remove leading slash if present since target_url already has trailing slash
        let path_and_query = path_and_query.strip_prefix('/').unwrap_or(path_and_query);
        let uri = format!("{}{}", self.target_url, path_and_query);
        trace!(method = ?method, uri = %uri, "Proxying request");
        trace!(headers = ?headers, "Original headers");

        // Convert the request body to bytes
        let body_bytes = req.into_body().collect().await?.to_bytes();
        trace!(body_length = body_bytes.len(), "Request body collected");
        
        // Build the new request
        let mut builder = Request::builder()
            .uri(uri)
            .method(method);
            
        // Copy all headers except host
        let headers_mut = builder.headers_mut().unwrap();
        for (key, value) in headers.iter() {
            if key.as_str().to_lowercase() != "host" {
                headers_mut.insert(key, value.clone());
            }
        }
        trace!(forwarded_headers = ?headers_mut, "Forwarding headers");
        
        let req = builder.body(Full::new(body_bytes))?;

        // Forward the request and wait for response
        let response = self.client.request(req).await?;
        trace!(
            status = ?response.status(),
            headers = ?response.headers(),
            "Received response"
        );
        
        // Convert the response body
        let (parts, body) = response.into_parts();
        let body = body.collect().await?.to_bytes();
        trace!(body_length = body.len(), "Response body collected");
        Ok(Response::from_parts(parts, Body::from(body)))
    }

    pub fn router(self) -> Router {
        Router::new().fallback(any(move |req| {
            let proxy = self.clone();
            async move {
                match proxy.proxy_request(req).await {
                    Ok(response) => response,
                    Err(err) => {
                        error!(%err, "Proxy error occurred");
                        Response::builder()
                            .status(500)
                            .body(Body::from(format!("Proxy error: {}", err)))
                            .unwrap()
                    }
                }
            }
        }))
    }
}

impl Clone for ReverseProxy {
    fn clone(&self) -> Self {
        Self {
            target_url: self.target_url.clone(),
            client: Client::builder(TokioExecutor::new()).build(HttpConnector::new()),
        }
    }
}
