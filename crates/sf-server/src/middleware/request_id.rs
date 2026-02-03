//! Request ID middleware.
//!
//! Generates a UUID for each request (or extracts an existing `x-request-id`
//! header), stores it in a tracing span, and returns it in the response.

use axum::http::{HeaderName, HeaderValue, Request};
use axum::middleware::Next;
use axum::response::Response;
use uuid::Uuid;

/// Header name used for the request identifier.
pub static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Middleware that generates or extracts a request ID.
pub async fn request_id_middleware(
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let id = request
        .headers()
        .get(&X_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Store in request extensions so handlers can access it.
    request.extensions_mut().insert(RequestId(id.clone()));

    let span = tracing::info_span!("request", request_id = %id);
    let _guard = span.enter();

    let mut response = next.run(request).await;

    if let Ok(val) = HeaderValue::from_str(&id) {
        response.headers_mut().insert(X_REQUEST_ID.clone(), val);
    }

    response
}

/// Extracted request ID from the request extensions.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);
