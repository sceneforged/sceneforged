//! Invitation management routes.

use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct InvitationResponse {
    pub id: String,
    pub code: String,
    pub role: String,
    pub created_by: String,
    pub created_at: String,
    pub expires_at: String,
    pub used_at: Option<String>,
    pub used_by: Option<String>,
}

impl From<sf_db::models::Invitation> for InvitationResponse {
    fn from(inv: sf_db::models::Invitation) -> Self {
        Self {
            id: inv.id.to_string(),
            code: inv.code,
            role: inv.role,
            created_by: inv.created_by.to_string(),
            created_at: inv.created_at,
            expires_at: inv.expires_at,
            used_at: inv.used_at,
            used_by: inv.used_by.map(|id| id.to_string()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    pub role: Option<String>,
    pub expires_in_days: Option<i64>,
}

/// POST /api/admin/invitations -- create a new invitation.
pub async fn create_invitation(
    State(ctx): State<AppContext>,
    Extension(user_id): Extension<sf_core::UserId>,
    Json(payload): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<InvitationResponse>), AppError> {
    let role = payload.role.as_deref().unwrap_or("user");
    let days = payload.expires_in_days.unwrap_or(7);
    let expires_at = (Utc::now() + Duration::days(days)).to_rfc3339();

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let inv = sf_db::queries::invitations::create_invitation(&conn, role, user_id, &expires_at)?;

    Ok((StatusCode::CREATED, Json(inv.into())))
}

/// GET /api/admin/invitations -- list all invitations.
pub async fn list_invitations(
    State(ctx): State<AppContext>,
) -> Result<Json<Vec<InvitationResponse>>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let invitations = sf_db::queries::invitations::list_invitations(&conn)?;
    Ok(Json(invitations.into_iter().map(Into::into).collect()))
}

/// DELETE /api/admin/invitations/{id} -- revoke an invitation.
pub async fn delete_invitation(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let inv_id: sf_core::InvitationId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid invitation ID".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let deleted = sf_db::queries::invitations::delete_invitation(&conn, inv_id)?;

    if !deleted {
        return Err(sf_core::Error::not_found("invitation", inv_id).into());
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub code: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub success: bool,
    pub token: String,
}

/// POST /api/auth/register -- register a new user with an invitation code.
pub async fn register(
    State(ctx): State<AppContext>,
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Look up invitation by code.
    let inv = sf_db::queries::invitations::get_invitation_by_code(&conn, &payload.code)?
        .ok_or_else(|| sf_core::Error::Validation("Invalid invitation code".into()))?;

    // Check if already used.
    if inv.used_at.is_some() {
        return Err(sf_core::Error::Conflict("Invitation already used".into()).into());
    }

    // Check expiration.
    let now = Utc::now();
    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&inv.expires_at) {
        if now > expires {
            return Err(sf_core::Error::Validation("Invitation has expired".into()).into());
        }
    }

    // Create user with bcrypt-hashed password.
    let hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| sf_core::Error::Internal(format!("bcrypt error: {e}")))?;
    let user = sf_db::queries::users::create_user(&conn, &payload.username, &hash, &inv.role)?;

    // Redeem invitation.
    sf_db::queries::invitations::redeem_invitation(&conn, &payload.code, user.id)?;

    // Create auth token.
    let token = uuid::Uuid::new_v4().to_string();
    let expires = now + Duration::hours(ctx.config.auth.session_timeout_hours as i64);
    let expires_str = expires.to_rfc3339();
    sf_db::queries::auth::create_token(&conn, user.id, &token, &expires_str)?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            success: true,
            token,
        }),
    ))
}
