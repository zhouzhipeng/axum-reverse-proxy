use axum::body::Body;
use http::StatusCode;
use http_body_util::BodyExt;
use std::convert::Infallible;
use std::time::Duration;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{Layer, Service};

#[derive(Clone)]
pub struct RetryLayer {
    attempts: usize,
    delay: Duration,
}

impl RetryLayer {
    pub fn new(attempts: usize) -> Self {
        Self {
            attempts,
            delay: Duration::from_millis(500),
        }
    }

    pub fn with_delay(attempts: usize, delay: Duration) -> Self {
        Self { attempts, delay }
    }
}

#[derive(Clone)]
pub struct Retry<S> {
    inner: S,
    attempts: usize,
    delay: Duration,
}

impl<S> Layer<S> for RetryLayer {
    type Service = Retry<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Retry {
            inner,
            attempts: self.attempts,
            delay: self.delay,
        }
    }
}

impl<S> Service<axum::http::Request<Body>> for Retry<S>
where
    S: Service<
            axum::http::Request<Body>,
            Response = axum::http::Response<Body>,
            Error = Infallible,
        > + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = axum::http::Response<Body>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::http::Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        let attempts = self.attempts;
        let delay = self.delay;
        Box::pin(async move {
            let (parts, body) = req.into_parts();
            let bytes = body.collect().await.unwrap().to_bytes();
            for attempt in 0..attempts {
                let req = axum::http::Request::from_parts(parts.clone(), Body::from(bytes.clone()));
                let res = inner.call(req).await?;
                if res.status() != StatusCode::BAD_GATEWAY || attempt == attempts - 1 {
                    return Ok(res);
                }
                tokio::time::sleep(delay).await;
            }
            unreachable!();
        })
    }
}
