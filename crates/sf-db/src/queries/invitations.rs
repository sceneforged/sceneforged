//! Invitation operations.

use chrono::Utc;
use rand::Rng;
use rusqlite::Connection;
use sf_core::{Error, InvitationId, Result, UserId};

use crate::models::Invitation;

const COLS: &str = "id, code, role, created_by, created_at, expires_at, used_at, used_by";

/// Generate a random 8-character alphanumeric code.
fn generate_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Create a new invitation.
pub fn create_invitation(
    conn: &Connection,
    role: &str,
    created_by: UserId,
    expires_at: &str,
) -> Result<Invitation> {
    let id = InvitationId::new();
    let code = generate_code();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO invitations (id, code, role, created_by, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            id.to_string(),
            &code,
            role,
            created_by.to_string(),
            &now,
            expires_at,
        ],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(Invitation {
        id,
        code,
        role: role.to_string(),
        created_by,
        created_at: now,
        expires_at: expires_at.to_string(),
        used_at: None,
        used_by: None,
    })
}

/// Get an invitation by its code.
pub fn get_invitation_by_code(conn: &Connection, code: &str) -> Result<Option<Invitation>> {
    let q = format!("SELECT {COLS} FROM invitations WHERE code = ?1");
    let result = conn.query_row(&q, [code], Invitation::from_row);
    match result {
        Ok(inv) => Ok(Some(inv)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::database(e.to_string())),
    }
}

/// Redeem an invitation: sets used_at and used_by.
/// Fails if the invitation is already used or expired.
pub fn redeem_invitation(
    conn: &Connection,
    code: &str,
    used_by: UserId,
) -> Result<Invitation> {
    let inv = get_invitation_by_code(conn, code)?
        .ok_or_else(|| Error::not_found("invitation", code))?;

    if inv.used_at.is_some() {
        return Err(Error::Conflict("Invitation already used".into()));
    }

    // Check expiration.
    let now = Utc::now();
    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&inv.expires_at) {
        if now > expires {
            return Err(Error::Validation("Invitation has expired".into()));
        }
    }

    let now_str = now.to_rfc3339();
    conn.execute(
        "UPDATE invitations SET used_at = ?1, used_by = ?2 WHERE id = ?3",
        rusqlite::params![&now_str, used_by.to_string(), inv.id.to_string()],
    )
    .map_err(|e| Error::database(e.to_string()))?;

    Ok(Invitation {
        used_at: Some(now_str),
        used_by: Some(used_by),
        ..inv
    })
}

/// List all invitations (for admin view).
pub fn list_invitations(conn: &Connection) -> Result<Vec<Invitation>> {
    let q = format!("SELECT {COLS} FROM invitations ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&q).map_err(|e| Error::database(e.to_string()))?;
    let rows = stmt
        .query_map([], Invitation::from_row)
        .map_err(|e| Error::database(e.to_string()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(rows)
}

/// Delete an invitation by ID. Returns true if a row was deleted.
pub fn delete_invitation(conn: &Connection, id: InvitationId) -> Result<bool> {
    let n = conn
        .execute("DELETE FROM invitations WHERE id = ?1", [id.to_string()])
        .map_err(|e| Error::database(e.to_string()))?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_memory_pool;
    use crate::queries::users;
    use chrono::Duration;

    #[test]
    fn create_and_list() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let user = users::create_user(&conn, "admin", "h", "admin").unwrap();

        let expires = (Utc::now() + Duration::days(7)).to_rfc3339();
        let inv = create_invitation(&conn, "user", user.id, &expires).unwrap();
        assert_eq!(inv.code.len(), 8);
        assert_eq!(inv.role, "user");

        let list = list_invitations(&conn).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn redeem_and_expire() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let admin = users::create_user(&conn, "admin", "h", "admin").unwrap();
        let new_user = users::create_user(&conn, "newuser", "h", "user").unwrap();

        let expires = (Utc::now() + Duration::days(7)).to_rfc3339();
        let inv = create_invitation(&conn, "user", admin.id, &expires).unwrap();

        let redeemed = redeem_invitation(&conn, &inv.code, new_user.id).unwrap();
        assert!(redeemed.used_at.is_some());
        assert_eq!(redeemed.used_by, Some(new_user.id));

        // Double-redeem should fail.
        assert!(redeem_invitation(&conn, &inv.code, new_user.id).is_err());
    }

    #[test]
    fn expired_invitation() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let admin = users::create_user(&conn, "admin", "h", "admin").unwrap();
        let new_user = users::create_user(&conn, "newuser", "h", "user").unwrap();

        // Create an already-expired invitation.
        let expires = (Utc::now() - Duration::days(1)).to_rfc3339();
        let inv = create_invitation(&conn, "user", admin.id, &expires).unwrap();

        assert!(redeem_invitation(&conn, &inv.code, new_user.id).is_err());
    }

    #[test]
    fn delete_invitation_test() {
        let pool = init_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let admin = users::create_user(&conn, "admin", "h", "admin").unwrap();

        let expires = (Utc::now() + Duration::days(7)).to_rfc3339();
        let inv = create_invitation(&conn, "user", admin.id, &expires).unwrap();

        assert!(delete_invitation(&conn, inv.id).unwrap());
        assert!(get_invitation_by_code(&conn, &inv.code).unwrap().is_none());
    }
}
