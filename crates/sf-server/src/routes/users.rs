//! User management routes (admin only).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub role: Option<String>,
    pub password: Option<String>,
}

/// GET /api/admin/users — list all users.
pub async fn list_users(
    State(ctx): State<AppContext>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let users = sf_db::queries::users::list_users(&conn)?;
    Ok(Json(
        users
            .into_iter()
            .map(|u| UserResponse {
                id: u.id.to_string(),
                username: u.username,
                role: u.role,
                created_at: u.created_at,
            })
            .collect(),
    ))
}

/// POST /api/admin/users — create a new user.
pub async fn create_user(
    State(ctx): State<AppContext>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    let hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| sf_core::Error::Internal(format!("bcrypt error: {e}")))?;
    let role = payload.role.as_deref().unwrap_or("user");

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let user = sf_db::queries::users::create_user(&conn, &payload.username, &hash, role)?;

    Ok((
        StatusCode::CREATED,
        Json(UserResponse {
            id: user.id.to_string(),
            username: user.username,
            role: user.role,
            created_at: user.created_at,
        }),
    ))
}

/// PUT /api/admin/users/{id} — update a user's role/password.
pub async fn update_user(
    State(ctx): State<AppContext>,
    Path(user_id): Path<String>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<StatusCode, AppError> {
    let id: sf_core::UserId = user_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid user_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;

    if let Some(role) = &payload.role {
        sf_db::queries::users::update_user_role(&conn, id, role)?;
    }

    if let Some(password) = &payload.password {
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| sf_core::Error::Internal(format!("bcrypt error: {e}")))?;
        sf_db::queries::users::update_password(&conn, id, &hash)?;
    }

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/admin/users/{id} — delete a user.
pub async fn delete_user(
    State(ctx): State<AppContext>,
    Path(user_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id: sf_core::UserId = user_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid user_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    sf_db::queries::users::delete_user(&conn, id)?;

    Ok(StatusCode::NO_CONTENT)
}
