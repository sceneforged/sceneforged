//! Governor-based rate limiting middleware.
//!
//! Provides configurable rate limiters for API routes (300/min) and webhook
//! routes (30/min).

use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;

/// A shared rate limiter instance.
pub type SharedLimiter = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>;

/// Create a rate limiter with the given requests-per-minute quota.
pub fn create_limiter(requests_per_minute: u32) -> SharedLimiter {
    let quota = Quota::per_minute(
        NonZeroU32::new(requests_per_minute).unwrap_or(NonZeroU32::new(300).unwrap()),
    );
    Arc::new(RateLimiter::direct(quota))
}

/// Rate limiting middleware. Returns 429 Too Many Requests when exceeded.
pub async fn rate_limit_middleware(
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    let limiter = request
        .extensions()
        .get::<SharedLimiter>()
        .cloned();

    if let Some(limiter) = limiter {
        if limiter.check().is_err() {
            return Err(
                (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response()
            );
        }
    }

    Ok(next.run(request).await)
}
