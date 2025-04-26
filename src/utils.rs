use tiny_http::Header;

/// Generate a Content-Type header easily
pub fn content_type_header(value: &str) -> Header {
    Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).unwrap()
}
