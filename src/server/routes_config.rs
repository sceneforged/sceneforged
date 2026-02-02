//! Configuration management API routes.

use crate::config::{persist, ArrConfig, ArrType, JellyfinConfig, MatchConditions, Rule};
use crate::server::AppContext;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

pub fn config_routes() -> Router<AppContext> {
    Router::new()
        // Rules CRUD
        .route("/config/rules", get(list_rules))
        .route("/config/rules", post(create_rule))
        .route("/config/rules/:name", get(get_rule))
        .route("/config/rules/:name", put(update_rule))
        .route("/config/rules/:name", delete(delete_rule))
        // Arrs CRUD
        .route("/config/arrs", get(list_arrs))
        .route("/config/arrs", post(create_arr))
        .route("/config/arrs/:name", get(get_arr))
        .route("/config/arrs/:name", put(update_arr))
        .route("/config/arrs/:name", delete(delete_arr))
        // Jellyfins CRUD
        .route("/config/jellyfins", get(list_jellyfins))
        .route("/config/jellyfins", post(create_jellyfin))
        .route("/config/jellyfins/:name", get(get_jellyfin))
        .route("/config/jellyfins/:name", put(update_jellyfin))
        .route("/config/jellyfins/:name", delete(delete_jellyfin))
        // Config operations
        .route("/config/reload", post(reload_config))
        .route("/config/validate", post(validate_config))
        // Path browsing
        .route("/config/browse", get(browse_paths))
}

// ============================================================================
// Rules CRUD
// ============================================================================

async fn list_rules(State(ctx): State<AppContext>) -> impl IntoResponse {
    let rules = ctx.rules.read();
    Json(rules.clone())
}

#[derive(Deserialize)]
struct CreateRuleRequest {
    name: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    priority: i32,
    match_conditions: MatchConditions,
    actions: Vec<crate::config::Action>,
}

async fn create_rule(
    State(ctx): State<AppContext>,
    Json(req): Json<CreateRuleRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Validate name
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Rule name cannot be empty".to_string(),
        ));
    }

    let mut rules = ctx.rules.write();

    // Check for duplicate name
    if rules.iter().any(|r| r.name.eq_ignore_ascii_case(&req.name)) {
        return Err((
            StatusCode::CONFLICT,
            format!("Rule '{}' already exists", req.name),
        ));
    }

    let rule = Rule {
        name: req.name.trim().to_string(),
        enabled: req.enabled,
        priority: req.priority,
        match_conditions: req.match_conditions,
        actions: req.actions,
        normalized: None,
    };

    rules.push(rule.clone());

    // Sort by priority
    rules.sort_by(|a, b| b.priority.cmp(&a.priority));

    // Persist to file
    if let Some(ref path) = ctx.config_path {
        if let Err(e) = persist::update_rules(path, &rules) {
            tracing::error!("Failed to persist rules: {}", e);
        }
    }

    Ok((StatusCode::CREATED, Json(rule)))
}

async fn get_rule(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
) -> Result<Json<Rule>, StatusCode> {
    let rules = ctx.rules.read();
    rules
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(&name))
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn update_rule(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
    Json(req): Json<CreateRuleRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut rules = ctx.rules.write();

    let idx = rules
        .iter()
        .position(|r| r.name.eq_ignore_ascii_case(&name))
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Rule '{}' not found", name)))?;

    // Check for duplicate name (if renaming)
    if !req.name.eq_ignore_ascii_case(&name)
        && rules.iter().any(|r| r.name.eq_ignore_ascii_case(&req.name))
    {
        return Err((
            StatusCode::CONFLICT,
            format!("Rule '{}' already exists", req.name),
        ));
    }

    let rule = Rule {
        name: req.name.trim().to_string(),
        enabled: req.enabled,
        priority: req.priority,
        match_conditions: req.match_conditions,
        actions: req.actions,
        normalized: None,
    };

    rules[idx] = rule.clone();

    // Sort by priority
    rules.sort_by(|a, b| b.priority.cmp(&a.priority));

    // Persist to file
    if let Some(ref path) = ctx.config_path {
        if let Err(e) = persist::update_rules(path, &rules) {
            tracing::error!("Failed to persist rules: {}", e);
        }
    }

    Ok(Json(rule))
}

async fn delete_rule(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut rules = ctx.rules.write();

    let len_before = rules.len();
    rules.retain(|r| !r.name.eq_ignore_ascii_case(&name));

    if rules.len() == len_before {
        return Err((StatusCode::NOT_FOUND, format!("Rule '{}' not found", name)));
    }

    // Persist to file
    if let Some(ref path) = ctx.config_path {
        if let Err(e) = persist::update_rules(path, &rules) {
            tracing::error!("Failed to persist rules: {}", e);
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Arrs CRUD
// ============================================================================

#[derive(Serialize)]
struct ArrResponse {
    name: String,
    #[serde(rename = "type")]
    arr_type: ArrType,
    url: String,
    enabled: bool,
    auto_rescan: bool,
    auto_rename: bool,
    // API key is not included in response for security
}

impl From<&ArrConfig> for ArrResponse {
    fn from(arr: &ArrConfig) -> Self {
        Self {
            name: arr.name.clone(),
            arr_type: arr.arr_type.clone(),
            url: arr.url.clone(),
            enabled: arr.enabled,
            auto_rescan: arr.auto_rescan,
            auto_rename: arr.auto_rename,
        }
    }
}

async fn list_arrs(State(ctx): State<AppContext>) -> impl IntoResponse {
    let arrs = ctx.arrs.read();
    let response: Vec<ArrResponse> = arrs.iter().map(ArrResponse::from).collect();
    Json(response)
}

#[derive(Deserialize)]
struct CreateArrRequest {
    name: String,
    #[serde(rename = "type")]
    arr_type: ArrType,
    url: String,
    api_key: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default = "default_true")]
    auto_rescan: bool,
    #[serde(default)]
    auto_rename: bool,
}

fn default_true() -> bool {
    true
}

async fn create_arr(
    State(ctx): State<AppContext>,
    Json(req): Json<CreateArrRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Validate
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Arr name cannot be empty".to_string(),
        ));
    }
    if req.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "URL cannot be empty".to_string()));
    }
    if req.api_key.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "API key cannot be empty".to_string(),
        ));
    }

    let mut arrs = ctx.arrs.write();

    // Check for duplicate name
    if arrs.iter().any(|a| a.name.eq_ignore_ascii_case(&req.name)) {
        return Err((
            StatusCode::CONFLICT,
            format!("Arr '{}' already exists", req.name),
        ));
    }

    let arr = ArrConfig {
        name: req.name.trim().to_string(),
        arr_type: req.arr_type,
        url: req.url.trim().to_string(),
        api_key: req.api_key,
        enabled: req.enabled,
        auto_rescan: req.auto_rescan,
        auto_rename: req.auto_rename,
    };

    arrs.push(arr.clone());

    // Persist to file
    if let Some(ref path) = ctx.config_path {
        if let Err(e) = persist::update_arrs(path, &arrs) {
            tracing::error!("Failed to persist arrs: {}", e);
        }
    }

    Ok((StatusCode::CREATED, Json(ArrResponse::from(&arr))))
}

async fn get_arr(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
) -> Result<Json<ArrResponse>, StatusCode> {
    let arrs = ctx.arrs.read();
    arrs.iter()
        .find(|a| a.name.eq_ignore_ascii_case(&name))
        .map(|a| Json(ArrResponse::from(a)))
        .ok_or(StatusCode::NOT_FOUND)
}

#[derive(Deserialize)]
struct UpdateArrRequest {
    name: Option<String>,
    #[serde(rename = "type")]
    arr_type: Option<ArrType>,
    url: Option<String>,
    api_key: Option<String>,
    enabled: Option<bool>,
    auto_rescan: Option<bool>,
    auto_rename: Option<bool>,
}

async fn update_arr(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
    Json(req): Json<UpdateArrRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut arrs = ctx.arrs.write();

    let idx = arrs
        .iter()
        .position(|a| a.name.eq_ignore_ascii_case(&name))
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Arr '{}' not found", name)))?;

    // Check for duplicate name (if renaming)
    if let Some(ref new_name) = req.name {
        if !new_name.eq_ignore_ascii_case(&name)
            && arrs.iter().any(|a| a.name.eq_ignore_ascii_case(new_name))
        {
            return Err((
                StatusCode::CONFLICT,
                format!("Arr '{}' already exists", new_name),
            ));
        }
    }

    // Apply updates
    let arr = &mut arrs[idx];
    if let Some(new_name) = req.name {
        arr.name = new_name.trim().to_string();
    }
    if let Some(arr_type) = req.arr_type {
        arr.arr_type = arr_type;
    }
    if let Some(url) = req.url {
        arr.url = url.trim().to_string();
    }
    if let Some(api_key) = req.api_key {
        arr.api_key = api_key;
    }
    if let Some(enabled) = req.enabled {
        arr.enabled = enabled;
    }
    if let Some(auto_rescan) = req.auto_rescan {
        arr.auto_rescan = auto_rescan;
    }
    if let Some(auto_rename) = req.auto_rename {
        arr.auto_rename = auto_rename;
    }

    let response = ArrResponse::from(&arrs[idx]);

    // Persist to file
    if let Some(ref path) = ctx.config_path {
        if let Err(e) = persist::update_arrs(path, &arrs) {
            tracing::error!("Failed to persist arrs: {}", e);
        }
    }

    Ok(Json(response))
}

async fn delete_arr(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut arrs = ctx.arrs.write();

    let len_before = arrs.len();
    arrs.retain(|a| !a.name.eq_ignore_ascii_case(&name));

    if arrs.len() == len_before {
        return Err((StatusCode::NOT_FOUND, format!("Arr '{}' not found", name)));
    }

    // Persist to file
    if let Some(ref path) = ctx.config_path {
        if let Err(e) = persist::update_arrs(path, &arrs) {
            tracing::error!("Failed to persist arrs: {}", e);
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Jellyfins CRUD
// ============================================================================

#[derive(Serialize)]
struct JellyfinResponse {
    name: String,
    url: String,
    enabled: bool,
    // API key is not included in response for security
}

impl From<&JellyfinConfig> for JellyfinResponse {
    fn from(j: &JellyfinConfig) -> Self {
        Self {
            name: j.name.clone(),
            url: j.url.clone(),
            enabled: j.enabled,
        }
    }
}

async fn list_jellyfins(State(ctx): State<AppContext>) -> impl IntoResponse {
    let jellyfins = ctx.jellyfins.read();
    let response: Vec<JellyfinResponse> = jellyfins.iter().map(JellyfinResponse::from).collect();
    Json(response)
}

#[derive(Deserialize)]
struct CreateJellyfinRequest {
    name: String,
    url: String,
    api_key: String,
    #[serde(default)]
    enabled: bool,
}

async fn create_jellyfin(
    State(ctx): State<AppContext>,
    Json(req): Json<CreateJellyfinRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Validate
    if req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Name cannot be empty".to_string()));
    }
    if req.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "URL cannot be empty".to_string()));
    }
    if req.api_key.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "API key cannot be empty".to_string(),
        ));
    }

    let mut jellyfins = ctx.jellyfins.write();

    // Check for duplicate name
    if jellyfins
        .iter()
        .any(|j| j.name.eq_ignore_ascii_case(&req.name))
    {
        return Err((
            StatusCode::CONFLICT,
            format!("Jellyfin '{}' already exists", req.name),
        ));
    }

    let jellyfin = JellyfinConfig {
        name: req.name.trim().to_string(),
        url: req.url.trim().to_string(),
        api_key: req.api_key,
        enabled: req.enabled,
    };

    jellyfins.push(jellyfin.clone());

    // Persist to file
    if let Some(ref path) = ctx.config_path {
        if let Err(e) = persist::update_jellyfins(path, &jellyfins) {
            tracing::error!("Failed to persist jellyfins: {}", e);
        }
    }

    Ok((StatusCode::CREATED, Json(JellyfinResponse::from(&jellyfin))))
}

async fn get_jellyfin(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
) -> Result<Json<JellyfinResponse>, StatusCode> {
    let jellyfins = ctx.jellyfins.read();
    jellyfins
        .iter()
        .find(|j| j.name.eq_ignore_ascii_case(&name))
        .map(|j| Json(JellyfinResponse::from(j)))
        .ok_or(StatusCode::NOT_FOUND)
}

#[derive(Deserialize)]
struct UpdateJellyfinRequest {
    name: Option<String>,
    url: Option<String>,
    api_key: Option<String>,
    enabled: Option<bool>,
}

async fn update_jellyfin(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
    Json(req): Json<UpdateJellyfinRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut jellyfins = ctx.jellyfins.write();

    let idx = jellyfins
        .iter()
        .position(|j| j.name.eq_ignore_ascii_case(&name))
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("Jellyfin '{}' not found", name),
            )
        })?;

    // Check for duplicate name (if renaming)
    if let Some(ref new_name) = req.name {
        if !new_name.eq_ignore_ascii_case(&name)
            && jellyfins
                .iter()
                .any(|j| j.name.eq_ignore_ascii_case(new_name))
        {
            return Err((
                StatusCode::CONFLICT,
                format!("Jellyfin '{}' already exists", new_name),
            ));
        }
    }

    // Apply updates
    let jellyfin = &mut jellyfins[idx];
    if let Some(new_name) = req.name {
        jellyfin.name = new_name.trim().to_string();
    }
    if let Some(url) = req.url {
        jellyfin.url = url.trim().to_string();
    }
    if let Some(api_key) = req.api_key {
        jellyfin.api_key = api_key;
    }
    if let Some(enabled) = req.enabled {
        jellyfin.enabled = enabled;
    }

    let response = JellyfinResponse::from(&jellyfins[idx]);

    // Persist to file
    if let Some(ref path) = ctx.config_path {
        if let Err(e) = persist::update_jellyfins(path, &jellyfins) {
            tracing::error!("Failed to persist jellyfins: {}", e);
        }
    }

    Ok(Json(response))
}

async fn delete_jellyfin(
    State(ctx): State<AppContext>,
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut jellyfins = ctx.jellyfins.write();

    let len_before = jellyfins.len();
    jellyfins.retain(|j| !j.name.eq_ignore_ascii_case(&name));

    if jellyfins.len() == len_before {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Jellyfin '{}' not found", name),
        ));
    }

    // Persist to file
    if let Some(ref path) = ctx.config_path {
        if let Err(e) = persist::update_jellyfins(path, &jellyfins) {
            tracing::error!("Failed to persist jellyfins: {}", e);
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Config Operations
// ============================================================================

async fn reload_config(
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let path = ctx.config_path.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "No config file path configured".to_string(),
        )
    })?;

    let config = crate::config::load_config(path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to reload config: {}", e),
        )
    })?;

    // Update mutable config sections
    {
        let mut rules = ctx.rules.write();
        *rules = config.rules;
    }
    {
        let mut arrs = ctx.arrs.write();
        *arrs = config.arrs;
    }
    {
        let mut jellyfins = ctx.jellyfins.write();
        *jellyfins = config.jellyfins;
    }

    Ok(Json(serde_json::json!({
        "status": "reloaded",
        "message": "Configuration reloaded successfully"
    })))
}

#[derive(Deserialize)]
struct ValidateRequest {
    rules: Option<Vec<Rule>>,
    arrs: Option<Vec<ArrConfig>>,
    jellyfins: Option<Vec<JellyfinConfig>>,
}

#[derive(Serialize)]
struct ValidationResult {
    valid: bool,
    errors: Vec<String>,
}

async fn validate_config(Json(req): Json<ValidateRequest>) -> impl IntoResponse {
    let mut errors = Vec::new();

    // Validate rules
    if let Some(ref rules) = req.rules {
        for rule in rules {
            if rule.name.trim().is_empty() {
                errors.push("Rule name cannot be empty".to_string());
            }
            if rule.enabled && rule.actions.is_empty() {
                errors.push(format!(
                    "Rule '{}' is enabled but has no actions",
                    rule.name
                ));
            }
        }

        // Check for duplicate names
        let mut names: Vec<&str> = rules.iter().map(|r| r.name.as_str()).collect();
        names.sort();
        for window in names.windows(2) {
            if window[0].eq_ignore_ascii_case(window[1]) {
                errors.push(format!("Duplicate rule name: '{}'", window[0]));
            }
        }
    }

    // Validate arrs
    if let Some(ref arrs) = req.arrs {
        for arr in arrs {
            if arr.name.trim().is_empty() {
                errors.push("Arr name cannot be empty".to_string());
            }
            if arr.url.trim().is_empty() {
                errors.push(format!("Arr '{}' has empty URL", arr.name));
            }
            if arr.enabled && arr.api_key.is_empty() {
                errors.push(format!("Arr '{}' is enabled but has no API key", arr.name));
            }
        }

        // Check for duplicate names
        let mut names: Vec<&str> = arrs.iter().map(|a| a.name.as_str()).collect();
        names.sort();
        for window in names.windows(2) {
            if window[0].eq_ignore_ascii_case(window[1]) {
                errors.push(format!("Duplicate arr name: '{}'", window[0]));
            }
        }
    }

    // Validate jellyfins
    if let Some(ref jellyfins) = req.jellyfins {
        for jellyfin in jellyfins {
            if jellyfin.name.trim().is_empty() {
                errors.push("Jellyfin name cannot be empty".to_string());
            }
            if jellyfin.url.trim().is_empty() {
                errors.push(format!("Jellyfin '{}' has empty URL", jellyfin.name));
            }
            if jellyfin.enabled && jellyfin.api_key.is_empty() {
                errors.push(format!(
                    "Jellyfin '{}' is enabled but has no API key",
                    jellyfin.name
                ));
            }
        }

        // Check for duplicate names
        let mut names: Vec<&str> = jellyfins.iter().map(|j| j.name.as_str()).collect();
        names.sort();
        for window in names.windows(2) {
            if window[0].eq_ignore_ascii_case(window[1]) {
                errors.push(format!("Duplicate jellyfin name: '{}'", window[0]));
            }
        }
    }

    Json(ValidationResult {
        valid: errors.is_empty(),
        errors,
    })
}

// ============================================================================
// Path Browsing
// ============================================================================

#[derive(Deserialize)]
struct BrowseQuery {
    path: Option<String>,
    search: Option<String>,
}

#[derive(Serialize)]
struct DirEntry {
    name: String,
    path: String,
    is_dir: bool,
}

async fn browse_paths(Query(params): Query<BrowseQuery>) -> impl IntoResponse {
    let base = params.path.unwrap_or_else(|| "/".to_string());
    let path = PathBuf::from(&base);

    if !path.exists() || !path.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid path"})),
        )
            .into_response();
    }

    let mut entries: Vec<DirEntry> = vec![];
    if let Ok(read_dir) = std::fs::read_dir(&path) {
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden files/directories
            if name.starts_with('.') {
                continue;
            }
            if let Some(ref search) = params.search {
                if !name.to_lowercase().contains(&search.to_lowercase()) {
                    continue;
                }
            }
            // Only show directories for library paths
            if entry.path().is_dir() {
                entries.push(DirEntry {
                    name,
                    path: entry.path().to_string_lossy().to_string(),
                    is_dir: true,
                });
            }
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Json(entries).into_response()
}
