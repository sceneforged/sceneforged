//! Configuration management route handlers.
//!
//! All serialization of `Rule` (which contains the recursive `Expr` type) is
//! done through string intermediaries to avoid a rustc ICE caused by deep
//! generic monomorphization of serde's `Serializer`/`Deserializer` traits.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// GET /api/config/rules
#[utoipa::path(
    get,
    path = "/api/config/rules",
    responses(
        (status = 200, description = "List processing rules", body = Vec<serde_json::Value>)
    )
)]
pub async fn get_rules(State(ctx): State<AppContext>) -> Result<impl IntoResponse, AppError> {
    let rules = ctx.config_store.get_rules();
    // Use sf_rules helpers to keep serde monomorphization in sf-rules crate.
    let value = sf_rules::rules_to_value(&rules)
        .map_err(|e| sf_core::Error::Internal(format!("serialize rules: {e}")))?;
    Ok(Json(value))
}

/// PUT /api/config/rules
#[utoipa::path(
    put,
    path = "/api/config/rules",
    request_body = Vec<serde_json::Value>,
    responses(
        (status = 200, description = "Rules updated", body = Vec<serde_json::Value>)
    )
)]
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

// ---------------------------------------------------------------------------
// Arrs
// ---------------------------------------------------------------------------

/// GET /api/config/arrs
pub async fn get_arrs(State(ctx): State<AppContext>) -> impl IntoResponse {
    let arrs = ctx.config_store.arrs.read().clone();
    Json(serde_json::to_value(&arrs).unwrap_or_default())
}

/// POST /api/config/arrs
pub async fn create_arr(
    State(ctx): State<AppContext>,
    Json(arr): Json<sf_core::config::ArrConfig>,
) -> Result<impl IntoResponse, AppError> {
    if arr.name.is_empty() {
        return Err(sf_core::Error::Validation("name is required".into()).into());
    }

    {
        let mut arrs = ctx.config_store.arrs.write();
        if arrs.iter().any(|a| a.name == arr.name) {
            return Err(sf_core::Error::Conflict(format!("arr '{}' already exists", arr.name)).into());
        }
        arrs.push(arr.clone());
    }
    ctx.config_store.persist();

    Ok((StatusCode::CREATED, Json(serde_json::to_value(&arr).unwrap_or_default())))
}

/// PUT /api/config/arrs/:name
pub async fn update_arr(
    State(ctx): State<AppContext>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(arr): Json<sf_core::config::ArrConfig>,
) -> Result<impl IntoResponse, AppError> {
    {
        let mut arrs = ctx.config_store.arrs.write();
        let existing = arrs.iter_mut().find(|a| a.name == name);
        match existing {
            Some(a) => *a = arr.clone(),
            None => return Err(sf_core::Error::not_found("arr", &name).into()),
        }
    }
    ctx.config_store.persist();

    Ok(Json(serde_json::to_value(&arr).unwrap_or_default()))
}

/// DELETE /api/config/arrs/:name
pub async fn delete_arr(
    State(ctx): State<AppContext>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    {
        let mut arrs = ctx.config_store.arrs.write();
        let initial_len = arrs.len();
        arrs.retain(|a| a.name != name);
        if arrs.len() == initial_len {
            return Err(sf_core::Error::not_found("arr", &name).into());
        }
    }
    ctx.config_store.persist();

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/config/arrs/:name/test
pub async fn test_arr(
    State(ctx): State<AppContext>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let arr = {
        let arrs = ctx.config_store.arrs.read();
        arrs.iter()
            .find(|a| a.name == name)
            .cloned()
            .ok_or_else(|| sf_core::Error::not_found("arr", &name))?
    };

    // Try to connect to the arr's system/status endpoint.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| sf_core::Error::Internal(format!("HTTP client error: {e}")))?;

    let url = format!("{}/api/v3/system/status", arr.url.trim_end_matches('/'));
    let result = client
        .get(&url)
        .header("X-Api-Key", &arr.api_key)
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => Ok(Json(
            serde_json::json!({"success": true, "message": "Connection successful"}),
        )),
        Ok(resp) => Ok(Json(serde_json::json!({
            "success": false,
            "message": format!("Server returned HTTP {}", resp.status())
        }))),
        Err(e) => Ok(Json(serde_json::json!({
            "success": false,
            "message": format!("Connection failed: {e}")
        }))),
    }
}

// ---------------------------------------------------------------------------
// Jellyfins
// ---------------------------------------------------------------------------

/// GET /api/config/jellyfins
pub async fn get_jellyfins(State(ctx): State<AppContext>) -> impl IntoResponse {
    let jfs = ctx.config_store.jellyfins.read().clone();
    Json(serde_json::to_value(&jfs).unwrap_or_default())
}

/// POST /api/config/jellyfins
pub async fn create_jellyfin(
    State(ctx): State<AppContext>,
    Json(jf): Json<sf_core::config::JellyfinConfig>,
) -> Result<impl IntoResponse, AppError> {
    if jf.name.is_empty() {
        return Err(sf_core::Error::Validation("name is required".into()).into());
    }

    {
        let mut jfs = ctx.config_store.jellyfins.write();
        if jfs.iter().any(|j| j.name == jf.name) {
            return Err(
                sf_core::Error::Conflict(format!("jellyfin '{}' already exists", jf.name)).into(),
            );
        }
        jfs.push(jf.clone());
    }
    ctx.config_store.persist();

    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(&jf).unwrap_or_default()),
    ))
}

/// PUT /api/config/jellyfins/:name
pub async fn update_jellyfin(
    State(ctx): State<AppContext>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(jf): Json<sf_core::config::JellyfinConfig>,
) -> Result<impl IntoResponse, AppError> {
    {
        let mut jfs = ctx.config_store.jellyfins.write();
        let existing = jfs.iter_mut().find(|j| j.name == name);
        match existing {
            Some(j) => *j = jf.clone(),
            None => return Err(sf_core::Error::not_found("jellyfin", &name).into()),
        }
    }
    ctx.config_store.persist();

    Ok(Json(serde_json::to_value(&jf).unwrap_or_default()))
}

/// DELETE /api/config/jellyfins/:name
pub async fn delete_jellyfin(
    State(ctx): State<AppContext>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<impl IntoResponse, AppError> {
    {
        let mut jfs = ctx.config_store.jellyfins.write();
        let initial_len = jfs.len();
        jfs.retain(|j| j.name != name);
        if jfs.len() == initial_len {
            return Err(sf_core::Error::not_found("jellyfin", &name).into());
        }
    }
    ctx.config_store.persist();

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Conversion config
// ---------------------------------------------------------------------------

/// GET /api/config/conversion
pub async fn get_conversion(State(ctx): State<AppContext>) -> impl IntoResponse {
    let conv = ctx.config_store.conversion.read().clone();
    Json(serde_json::to_value(&conv).unwrap_or_default())
}

/// PUT /api/config/conversion
pub async fn update_conversion(
    State(ctx): State<AppContext>,
    Json(conv): Json<sf_core::config::ConversionConfig>,
) -> Result<impl IntoResponse, AppError> {
    *ctx.config_store.conversion.write() = conv.clone();
    ctx.config_store.persist();
    Ok(Json(serde_json::to_value(&conv).unwrap_or_default()))
}

// ---------------------------------------------------------------------------
// Config reload
// ---------------------------------------------------------------------------

/// POST /api/config/reload
pub async fn reload_config(State(ctx): State<AppContext>) -> impl IntoResponse {
    ctx.config_store.reload();
    Json(serde_json::json!({"status": "reloaded"}))
}

// ---------------------------------------------------------------------------
// Config validation
// ---------------------------------------------------------------------------

/// Validation result for the config.
#[derive(Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub warnings: Vec<String>,
}

/// POST /api/config/validate
///
/// Validate the current configuration and return any warnings.
pub async fn validate_config(
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse, AppError> {
    let warnings = ctx.config.validate();

    // Also check library paths exist.
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let libs = sf_db::queries::libraries::list_libraries(&conn)?;
    let mut all_warnings = warnings;

    for lib in &libs {
        for path in &lib.paths {
            if !std::path::Path::new(path).exists() {
                all_warnings.push(format!(
                    "Library '{}': path '{}' does not exist",
                    lib.name, path
                ));
            }
        }
    }

    // Check arr connectivity (URL format).
    let arrs = ctx.config_store.arrs.read();
    for arr in arrs.iter() {
        if arr.url.is_empty() {
            all_warnings.push(format!("Arr '{}': URL is empty", arr.name));
        }
        if arr.api_key.is_empty() {
            all_warnings.push(format!("Arr '{}': API key is empty", arr.name));
        }
    }

    // Check jellyfin configs.
    let jfs = ctx.config_store.jellyfins.read();
    for jf in jfs.iter() {
        if jf.url.is_empty() {
            all_warnings.push(format!("Jellyfin '{}': URL is empty", jf.name));
        }
        if jf.api_key.is_empty() {
            all_warnings.push(format!("Jellyfin '{}': API key is empty", jf.name));
        }
    }

    Ok(Json(ValidationResult {
        valid: all_warnings.is_empty(),
        warnings: all_warnings,
    }))
}

// ---------------------------------------------------------------------------
// Directory browser
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct BrowseParams {
    #[serde(default = "default_browse_path")]
    pub path: String,
}

fn default_browse_path() -> String {
    "/".into()
}

#[derive(Debug, Serialize)]
pub struct BrowseEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

/// GET /api/config/browse?path=
pub async fn browse_path(
    Query(params): Query<BrowseParams>,
) -> Result<impl IntoResponse, AppError> {
    let dir = std::path::Path::new(&params.path);

    if !dir.is_dir() {
        return Err(sf_core::Error::Validation(format!(
            "'{}' is not a directory",
            params.path
        ))
        .into());
    }

    let mut entries = Vec::new();

    // Add parent directory entry if not root.
    if let Some(parent) = dir.parent() {
        entries.push(BrowseEntry {
            name: "..".into(),
            path: parent.to_string_lossy().into_owned(),
            is_dir: true,
        });
    }

    let read_dir = std::fs::read_dir(dir)
        .map_err(|e| sf_core::Error::Validation(format!("Cannot read directory: {e}")))?;

    for entry in read_dir.flatten() {
        let metadata = entry.metadata();
        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);

        // Only show directories for path browsing.
        if !is_dir {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        // Skip hidden directories.
        if name.starts_with('.') {
            continue;
        }

        entries.push(BrowseEntry {
            name,
            path: entry.path().to_string_lossy().into_owned(),
            is_dir,
        });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Json(serde_json::json!({ "entries": entries })))
}
