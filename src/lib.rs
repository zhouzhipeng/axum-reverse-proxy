use axum::{
    body::Body,
    http::{Request, Uri},
    response::Response,
    routing::any,
    Router,
};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::{
    client::legacy::connect::HttpConnector, client::legacy::Client, rt::TokioExecutor,
};

pub struct ReverseProxy {
    target_url: Uri,
    client: Client<HttpConnector, Full<Bytes>>,
}

impl ReverseProxy {
    pub fn new(target_url: &str) -> Self {
        println!("Creating proxy with target URL: {}", target_url);
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

    pub async fn proxy_request(
        &self,
        req: Request<Body>,
    ) -> Result<Response, Box<dyn std::error::Error>> {
        // Extract parts we need before consuming the request
        let method = req.method().clone();
        let headers = req.headers().clone();
        let path_and_query = req.uri().path_and_query().map(|x| x.as_str()).unwrap_or("");

        // Remove leading slash if present since target_url already has trailing slash
        let path_and_query = path_and_query.strip_prefix('/').unwrap_or(path_and_query);
        let uri = format!("{}{}", self.target_url, path_and_query);
        println!("Proxying request: {} {}", method, uri);
        println!("Original headers: {:?}", headers);

        // Convert the request body to bytes
        let body_bytes = req.into_body().collect().await?.to_bytes();
        println!("Request body length: {}", body_bytes.len());

        // Build the new request
        let mut builder = Request::builder().uri(uri).method(method);

        // Copy all headers except host
        let headers_mut = builder.headers_mut().unwrap();
        for (key, value) in headers.iter() {
            if key.as_str().to_lowercase() != "host" {
                headers_mut.insert(key, value.clone());
            }
        }
        println!("Forwarded headers: {:?}", headers_mut);

        let req = builder.body(Full::new(body_bytes))?;

        // Forward the request and wait for response
        let response = self.client.request(req).await?;
        println!("Response status: {}", response.status());
        println!("Response headers: {:?}", response.headers());

        // Convert the response body
        let (parts, body) = response.into_parts();
        let body = body.collect().await?.to_bytes();
        println!("Response body length: {}", body.len());
        Ok(Response::from_parts(parts, Body::from(body)))
    }

    pub fn router(self) -> Router {
        Router::new().fallback(any(move |req| {
            let proxy = self.clone();
            async move {
                match proxy.proxy_request(req).await {
                    Ok(response) => response,
                    Err(err) => {
                        println!("Proxy error: {}", err);
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
