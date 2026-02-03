//! Configuration management route handlers.
//!
//! All serialization of `Rule` (which contains the recursive `Expr` type) is
//! done through string intermediaries to avoid a rustc ICE caused by deep
//! generic monomorphization of serde's `Serializer`/`Deserializer` traits.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::context::AppContext;
use crate::error::AppError;

/// GET /api/config/rules
pub async fn get_rules(State(ctx): State<AppContext>) -> Result<impl IntoResponse, AppError> {
    let rules = ctx.config_store.get_rules();
    // Use sf_rules helpers to keep serde monomorphization in sf-rules crate.
    let value = sf_rules::rules_to_value(&rules)
        .map_err(|e| sf_core::Error::Internal(format!("serialize rules: {e}")))?;
    Ok(Json(value))
}

/// PUT /api/config/rules
pub async fn put_rules(
    State(ctx): State<AppContext>,
    body: String,
) -> Result<impl IntoResponse, AppError> {
    // Use sf_rules helpers to keep serde monomorphization in sf-rules crate.
    let rules = sf_rules::deserialize_rules(&body)
        .map_err(|e| sf_core::Error::Validation(format!("invalid rules JSON: {e}")))?;
    ctx.config_store.set_rules(rules.clone());
    ctx.config_store.persist();
    let value = sf_rules::rules_to_value(&rules)
        .map_err(|e| sf_core::Error::Internal(format!("serialize rules: {e}")))?;
    Ok(Json(value))
}

/// GET /api/config/arrs
pub async fn get_arrs(State(ctx): State<AppContext>) -> impl IntoResponse {
    let arrs = ctx.config_store.arrs.read().clone();
    Json(serde_json::to_value(&arrs).unwrap_or_default())
}
