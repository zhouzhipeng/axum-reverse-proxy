//! Utilities for creating HTTPS clients that accept invalid certificates.
//!
//! # Security Warning
//!
//! These utilities completely disable certificate validation, making connections
//! vulnerable to man-in-the-middle attacks. Only use in development/testing environments.

#[cfg(all(feature = "tls", not(feature = "native-tls")))]
use rustls::ClientConfig;

#[cfg(feature = "native-tls")]
use native_tls::TlsConnector;

/// Creates a rustls ClientConfig that accepts any certificate.
///
/// # Security Warning
///
/// This configuration will accept ANY certificate, including:
/// - Self-signed certificates
/// - Expired certificates  
/// - Certificates with wrong hostnames
/// - Certificates from untrusted CAs
///
/// Only use this for development or testing!
#[cfg(all(feature = "tls", not(feature = "native-tls")))]
pub fn create_dangerous_rustls_config() -> ClientConfig {
    use std::sync::Arc;

    #[derive(Debug)]
    struct NoCertificateVerification;

    impl rustls::client::danger::ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            // Support all signature schemes
            vec![
                rustls::SignatureScheme::RSA_PKCS1_SHA256,
                rustls::SignatureScheme::RSA_PKCS1_SHA384,
                rustls::SignatureScheme::RSA_PKCS1_SHA512,
                rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
                rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
                rustls::SignatureScheme::RSA_PSS_SHA256,
                rustls::SignatureScheme::RSA_PSS_SHA384,
                rustls::SignatureScheme::RSA_PSS_SHA512,
                rustls::SignatureScheme::ED25519,
                rustls::SignatureScheme::RSA_PKCS1_SHA1,
                rustls::SignatureScheme::ECDSA_SHA1_Legacy,
            ]
        }
    }

    ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
        .with_no_client_auth()
}

/// Creates a native-tls TlsConnector that accepts any certificate.
///
/// # Security Warning
///
/// This connector will accept ANY certificate. Only use for development or testing!
#[cfg(feature = "native-tls")]
pub fn create_dangerous_native_tls_connector() -> Result<TlsConnector, native_tls::Error> {
    TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true)
        .build()
}
