use crate::proxy::ReverseProxy;
use axum::routing::Router;
use hyper_util::client::legacy::connect::Connect;

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
impl<C, S> From<ReverseProxy<C>> for Router<S>
where
    C: Connect + Clone + Send + Sync + 'static,
    S: Send + Sync + Clone + 'static,
{
    fn from(proxy: ReverseProxy<C>) -> Self {
        let path = proxy.path().to_string();
        let proxy_router = Router::<S>::new().fallback_service(proxy);

        if ["", "/"].contains(&path.as_str()) {
            proxy_router
        } else {
            Router::new().nest(&path, proxy_router)
        }
    }
}

use crate::balanced_proxy::BalancedProxy;

impl<C, S> From<BalancedProxy<C>> for Router<S>
where
    C: Connect + Clone + Send + Sync + 'static,
    S: Send + Sync + Clone + 'static,
{
    fn from(proxy: BalancedProxy<C>) -> Self {
        let path = proxy.path().to_string();
        let proxy_router = Router::<S>::new().fallback_service(proxy);

        if ["", "/"].contains(&path.as_str()) {
            proxy_router
        } else {
            Router::new().nest(&path, proxy_router)
        }
    }
}
