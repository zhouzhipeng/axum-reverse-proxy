use axum::{
    Router,
    routing::any,
    http::{Request, Uri, HeaderMap},
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

pub struct ReverseProxy {
    target_url: Uri,
    client: Client<HttpConnector, Full<Bytes>>,
}

impl ReverseProxy {
    pub fn new(target_url: &str) -> Self {
        Self {
            target_url: target_url.parse().expect("Invalid target URL"),
            client: Client::builder(TokioExecutor::new()).build(HttpConnector::new()),
        }
    }

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
            
        let uri = format!("{}{}", self.target_url, path_and_query);

        // Convert the request body to bytes
        let body_bytes = req.into_body().collect().await?.to_bytes();
        
        // Build the new request
        let mut builder = Request::builder()
            .uri(uri)
            .method(method);
            
        // Copy headers
        *builder.headers_mut().unwrap() = headers;
        
        let req = builder.body(Full::new(body_bytes))?;

        // Forward the request and wait for response
        let response = self.client.request(req).await?;
        
        // Convert the response body
        let (parts, body) = response.into_parts();
        let body = body.collect().await?.to_bytes();
        Ok(Response::from_parts(parts, Body::from(body)))
    }

    pub fn router(self) -> Router {
        Router::new().fallback(any(move |req| {
            let proxy = self.clone();
            async move {
                match proxy.proxy_request(req).await {
                    Ok(response) => response,
                    Err(err) => {
                        // In a production environment, you'd want to handle this more gracefully
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
