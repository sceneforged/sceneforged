//! Stream session tracking for Jellyfin-compatible clients.
//!
//! Tracks active streaming sessions with automatic cleanup of inactive sessions.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use sceneforged_common::types::Profile;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// A streaming session from a Jellyfin-compatible client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSession {
    /// Unique session identifier (UUID).
    pub id: String,
    /// Client IP address.
    pub client_ip: String,
    /// Media item ID being streamed.
    pub item_id: i64,
    /// Profile being served (A, B, or C).
    pub profile: Profile,
    /// Session start timestamp.
    pub started_at: DateTime<Utc>,
    /// Last heartbeat timestamp (for activity tracking).
    pub last_seen: DateTime<Utc>,
}

/// Thread-safe session manager for tracking active streams.
#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, StreamSession>>,
    /// Duration after which a session is considered expired.
    expiry_duration: Duration,
}

impl SessionManager {
    /// Create a new session manager.
    ///
    /// # Arguments
    /// * `expiry_secs` - Number of seconds before a session is considered expired.
    pub fn new(expiry_secs: u64) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            expiry_duration: Duration::from_secs(expiry_secs),
        }
    }

    /// Register a new streaming session.
    ///
    /// # Arguments
    /// * `client_ip` - IP address of the client.
    /// * `item_id` - Media item ID being streamed.
    /// * `profile` - Profile being served to the client.
    ///
    /// # Returns
    /// The unique session ID.
    pub fn register_session(&self, client_ip: String, item_id: i64, profile: Profile) -> String {
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let session = StreamSession {
            id: session_id.clone(),
            client_ip,
            item_id,
            profile,
            started_at: now,
            last_seen: now,
        };

        self.sessions.insert(session_id.clone(), session);
        tracing::info!(
            session_id = %session_id,
            item_id = item_id,
            profile = %profile,
            "Registered new stream session"
        );

        session_id
    }

    /// Update the heartbeat for an active session.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to update.
    ///
    /// # Returns
    /// * `Ok(())` if the session was found and updated.
    /// * `Err(String)` if the session was not found.
    pub fn heartbeat(&self, session_id: &str) -> Result<(), String> {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.last_seen = Utc::now();
            tracing::debug!(session_id = %session_id, "Session heartbeat");
            Ok(())
        } else {
            Err(format!("Session not found: {}", session_id))
        }
    }

    /// End a streaming session.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to end.
    pub fn end_session(&self, session_id: &str) {
        if let Some((_, session)) = self.sessions.remove(session_id) {
            tracing::info!(
                session_id = %session_id,
                item_id = session.item_id,
                duration_secs = (Utc::now() - session.started_at).num_seconds(),
                "Ended stream session"
            );
        }
    }

    /// List all active sessions.
    ///
    /// # Returns
    /// A vector of all active streaming sessions.
    pub fn list_active_sessions(&self) -> Vec<StreamSession> {
        self.sessions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get a specific session by ID.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to retrieve.
    ///
    /// # Returns
    /// The session if found, or `None` if not found.
    pub fn get_session(&self, session_id: &str) -> Option<StreamSession> {
        self.sessions
            .get(session_id)
            .map(|entry| entry.value().clone())
    }

    /// Remove expired sessions based on last_seen timestamp.
    ///
    /// Sessions are considered expired if they haven't sent a heartbeat
    /// within the configured expiry duration (default 60 seconds).
    ///
    /// # Returns
    /// The number of sessions that were removed.
    pub fn cleanup_expired_sessions(&self) -> usize {
        let now = Utc::now();
        let expiry_duration_chrono = chrono::Duration::from_std(self.expiry_duration)
            .unwrap_or_else(|_| chrono::Duration::seconds(60));

        let mut removed_count = 0;
        self.sessions.retain(|session_id, session| {
            let elapsed = now - session.last_seen;
            if elapsed > expiry_duration_chrono {
                tracing::info!(
                    session_id = %session_id,
                    item_id = session.item_id,
                    inactive_secs = elapsed.num_seconds(),
                    "Expired session removed"
                );
                removed_count += 1;
                false
            } else {
                true
            }
        });

        if removed_count > 0 {
            tracing::debug!(removed = removed_count, "Cleaned up expired sessions");
        }

        removed_count
    }

    /// Get the number of active sessions.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Check if there are any active sessions.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        // Default: 60 second expiry
        Self::new(60)
    }
}

/// Start a background task that periodically cleans up expired sessions.
///
/// # Arguments
/// * `manager` - The session manager to clean up.
/// * `interval_secs` - How often to run cleanup (default: 30 seconds).
///
/// # Returns
/// A join handle for the background task.
pub fn start_cleanup_task(
    manager: SessionManager,
    interval_secs: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            manager.cleanup_expired_sessions();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_session() {
        let manager = SessionManager::new(60);
        let session_id = manager.register_session("192.168.1.1".to_string(), 123, Profile::A);

        assert!(!session_id.is_empty());
        assert_eq!(manager.len(), 1);

        let session = manager.get_session(&session_id).unwrap();
        assert_eq!(session.client_ip, "192.168.1.1");
        assert_eq!(session.item_id, 123);
        assert_eq!(session.profile, Profile::A);
    }

    #[test]
    fn test_heartbeat() {
        let manager = SessionManager::new(60);
        let session_id = manager.register_session("192.168.1.1".to_string(), 123, Profile::A);

        // Small delay to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        let session_before = manager.get_session(&session_id).unwrap();
        let result = manager.heartbeat(&session_id);
        assert!(result.is_ok());

        let session_after = manager.get_session(&session_id).unwrap();
        assert!(session_after.last_seen > session_before.last_seen);
    }

    #[test]
    fn test_heartbeat_nonexistent() {
        let manager = SessionManager::new(60);
        let result = manager.heartbeat("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_end_session() {
        let manager = SessionManager::new(60);
        let session_id = manager.register_session("192.168.1.1".to_string(), 123, Profile::A);

        assert_eq!(manager.len(), 1);
        manager.end_session(&session_id);
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn test_list_active_sessions() {
        let manager = SessionManager::new(60);
        manager.register_session("192.168.1.1".to_string(), 123, Profile::A);
        manager.register_session("192.168.1.2".to_string(), 456, Profile::B);

        let sessions = manager.list_active_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_cleanup_expired_sessions() {
        let manager = SessionManager::new(1); // 1 second expiry
        let session_id = manager.register_session("192.168.1.1".to_string(), 123, Profile::A);

        // Session should be active
        assert_eq!(manager.len(), 1);

        // Wait for session to expire
        std::thread::sleep(std::time::Duration::from_secs(2));

        let removed = manager.cleanup_expired_sessions();
        assert_eq!(removed, 1);
        assert_eq!(manager.len(), 0);
        assert!(manager.get_session(&session_id).is_none());
    }

    #[test]
    fn test_cleanup_keeps_active_sessions() {
        let manager = SessionManager::new(60);
        manager.register_session("192.168.1.1".to_string(), 123, Profile::A);

        let removed = manager.cleanup_expired_sessions();
        assert_eq!(removed, 0);
        assert_eq!(manager.len(), 1);
    }

    #[tokio::test]
    async fn test_cleanup_task() {
        let manager = SessionManager::new(1); // 1 second expiry
        manager.register_session("192.168.1.1".to_string(), 123, Profile::A);

        // Start cleanup task with short interval
        let handle = start_cleanup_task(manager.clone(), 1);

        // Wait for cleanup to run
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Session should be cleaned up
        assert_eq!(manager.len(), 0);

        // Clean up task
        handle.abort();
    }
}
