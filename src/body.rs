use bytes as bytes_crate;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use std::convert::Infallible;

/// Helper function to create a BoxBody from bytes
pub(crate) fn create_box_body(
    bytes: bytes_crate::Bytes,
) -> BoxBody<bytes_crate::Bytes, Box<dyn std::error::Error + Send + Sync>> {
    let full = Full::new(bytes);
    let mapped = full.map_err(|never: Infallible| match never {});
    let mapped = mapped.map_err(|_| {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "unreachable",
        )) as Box<dyn std::error::Error + Send + Sync>
    });
    BoxBody::new(mapped)
}
