//! Integration tests against badssl.com endpoints
//!
//! These tests verify that our dangerous HTTPS clients work correctly
//! with various types of invalid certificates in real-world scenarios.

#[cfg(any(feature = "tls", feature = "native-tls"))]
#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http_body_util::Empty;
    use hyper::{Request, StatusCode};
    use hyper_util::client::legacy::Client;
    use hyper_util::rt::TokioExecutor;

    #[cfg(feature = "native-tls")]
    use axum_reverse_proxy::create_dangerous_native_tls_connector;
    #[cfg(all(feature = "tls", not(feature = "native-tls")))]
    use axum_reverse_proxy::create_dangerous_rustls_config;

    // Helper to check if we got a successful response or a redirect
    fn is_success_or_redirect(status: StatusCode) -> bool {
        status.is_success() || status.is_redirection()
    }

    #[tokio::test]
    #[cfg(all(feature = "tls", not(feature = "native-tls")))]
    async fn test_rustls_self_signed_cert() {
        let tls_config = create_dangerous_rustls_config();
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .build();

        let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

        let response = client
            .request(
                Request::get("https://self-signed.badssl.com/")
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .expect("Should connect to self-signed cert");

        assert!(is_success_or_redirect(response.status()));
    }

    #[tokio::test]
    #[cfg(all(feature = "tls", not(feature = "native-tls")))]
    async fn test_rustls_wrong_host() {
        let tls_config = create_dangerous_rustls_config();
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .build();

        let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

        let response = client
            .request(
                Request::get("https://wrong.host.badssl.com/")
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .expect("Should connect to wrong hostname cert");

        assert!(is_success_or_redirect(response.status()));
    }

    #[tokio::test]
    #[cfg(all(feature = "tls", not(feature = "native-tls")))]
    async fn test_rustls_untrusted_root() {
        let tls_config = create_dangerous_rustls_config();
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .build();

        let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

        let response = client
            .request(
                Request::get("https://untrusted-root.badssl.com/")
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .expect("Should connect to untrusted root cert");

        assert!(is_success_or_redirect(response.status()));
    }

    #[tokio::test]
    #[cfg(all(feature = "tls", not(feature = "native-tls")))]
    async fn test_rustls_expired_cert() {
        let tls_config = create_dangerous_rustls_config();
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .build();

        let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

        let response = client
            .request(
                Request::get("https://expired.badssl.com/")
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .expect("Should connect to expired cert");

        assert!(is_success_or_redirect(response.status()));
    }

    #[tokio::test]
    #[cfg(feature = "native-tls")]
    async fn test_native_tls_self_signed_cert() {
        let tls = create_dangerous_native_tls_connector()
            .expect("Failed to create dangerous TLS connector");

        let mut http = hyper_util::client::legacy::connect::HttpConnector::new();
        http.enforce_http(false);

        let tls = tokio_native_tls::TlsConnector::from(tls);
        let https = hyper_tls::HttpsConnector::from((http, tls));

        let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

        let response = client
            .request(
                Request::get("https://self-signed.badssl.com/")
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .expect("Should connect to self-signed cert");

        assert!(is_success_or_redirect(response.status()));
    }

    #[tokio::test]
    #[cfg(feature = "native-tls")]
    async fn test_native_tls_wrong_host() {
        let tls = create_dangerous_native_tls_connector()
            .expect("Failed to create dangerous TLS connector");

        let mut http = hyper_util::client::legacy::connect::HttpConnector::new();
        http.enforce_http(false);

        let tls = tokio_native_tls::TlsConnector::from(tls);
        let https = hyper_tls::HttpsConnector::from((http, tls));

        let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

        let response = client
            .request(
                Request::get("https://wrong.host.badssl.com/")
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .expect("Should connect to wrong hostname cert");

        assert!(is_success_or_redirect(response.status()));
    }

    #[tokio::test]
    #[cfg(feature = "native-tls")]
    async fn test_native_tls_untrusted_root() {
        let tls = create_dangerous_native_tls_connector()
            .expect("Failed to create dangerous TLS connector");

        let mut http = hyper_util::client::legacy::connect::HttpConnector::new();
        http.enforce_http(false);

        let tls = tokio_native_tls::TlsConnector::from(tls);
        let https = hyper_tls::HttpsConnector::from((http, tls));

        let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

        let response = client
            .request(
                Request::get("https://untrusted-root.badssl.com/")
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .expect("Should connect to untrusted root cert");

        assert!(is_success_or_redirect(response.status()));
    }

    #[tokio::test]
    #[cfg(feature = "native-tls")]
    async fn test_native_tls_expired_cert() {
        let tls = create_dangerous_native_tls_connector()
            .expect("Failed to create dangerous TLS connector");

        let mut http = hyper_util::client::legacy::connect::HttpConnector::new();
        http.enforce_http(false);

        let tls = tokio_native_tls::TlsConnector::from(tls);
        let https = hyper_tls::HttpsConnector::from((http, tls));

        let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);

        let response = client
            .request(
                Request::get("https://expired.badssl.com/")
                    .body(Empty::<Bytes>::new())
                    .unwrap(),
            )
            .await
            .expect("Should connect to expired cert");

        assert!(is_success_or_redirect(response.status()));
    }
}
