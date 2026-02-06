//! Error-to-HTTP response conversion.
//!
//! Implements `IntoResponse` for [`sf_core::Error`] so that route handlers
//! can return `Result<T, sf_core::Error>` directly.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Wrapper so we can implement `IntoResponse` for an external type.
pub struct AppError {
    inner: sf_core::Error,
    request_id: Option<String>,
}

impl AppError {
    pub fn new(inner: sf_core::Error) -> Self {
        Self {
            inner,
            request_id: None,
        }
    }

    pub fn with_request_id(mut self, id: String) -> Self {
        self.request_id = Some(id);
        self
    }
}

impl From<sf_core::Error> for AppError {
    fn from(e: sf_core::Error) -> Self {
        Self::new(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.inner.http_status())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        if status.is_server_error() {
            tracing::error!(
                status = %status,
                error = %self.inner,
                "Server error in API handler"
            );
        }

        let code = match &self.inner {
            sf_core::Error::NotFound { .. } => "not_found",
            sf_core::Error::Unauthorized(_) => "unauthorized",
            sf_core::Error::Forbidden(_) => "forbidden",
            sf_core::Error::Validation(_) => "validation_error",
            sf_core::Error::Conflict(_) => "conflict",
            sf_core::Error::Database { .. } => "database_error",
            sf_core::Error::Io { .. } => "io_error",
            sf_core::Error::Tool { .. } => "tool_error",
            sf_core::Error::Probe(_) => "probe_error",
            sf_core::Error::Pipeline { .. } => "pipeline_error",
            sf_core::Error::Internal(_) => "internal_error",
        };

        let body = json!({
            "error": self.inner.to_string(),
            "code": code,
            "request_id": self.request_id,
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_produces_404() {
        let err = AppError::new(sf_core::Error::not_found("item", "abc"));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn unauthorized_produces_401() {
        let err = AppError::new(sf_core::Error::Unauthorized("bad token".into()));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn with_request_id() {
        let err = AppError::new(sf_core::Error::Internal("oops".into()))
            .with_request_id("req-123".into());
        assert_eq!(err.request_id.as_deref(), Some("req-123"));
    }
}
