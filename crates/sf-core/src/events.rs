//! Application event system for SSE broadcasting.
//!
//! [`EventBus`] wraps a `tokio::sync::broadcast` channel with a bounded
//! ring-buffer of recent events so that late-joining clients can catch up.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ids::{ConversionJobId, ItemId, JobId, LibraryId};

/// Maximum number of events retained in the ring buffer.
const MAX_RECENT_EVENTS: usize = 100;

// ---------------------------------------------------------------------------
// EventCategory
// ---------------------------------------------------------------------------

/// Audience category for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventCategory {
    /// Admin-only events (job processing, system status).
    Admin,
    /// User-facing events (library changes, item updates).
    User,
}

// ---------------------------------------------------------------------------
// EventPayload
// ---------------------------------------------------------------------------

/// Payload describing what happened.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventPayload {
    // -- Job lifecycle -------------------------------------------------------
    JobQueued {
        job_id: JobId,
    },
    JobStarted {
        job_id: JobId,
    },
    JobProgress {
        job_id: JobId,
        progress: f32,
        step: String,
    },
    JobCompleted {
        job_id: JobId,
    },
    JobFailed {
        job_id: JobId,
        error: String,
    },

    // -- Library lifecycle ---------------------------------------------------
    LibraryScanStarted {
        library_id: LibraryId,
    },
    LibraryScanProgress {
        library_id: LibraryId,
        files_found: u64,
        files_queued: u64,
        phase: String,
        files_total: u64,
        files_processed: u64,
    },
    LibraryScanComplete {
        library_id: LibraryId,
        files_found: u64,
        files_queued: u64,
        files_skipped: u64,
        errors: u64,
    },
    LibraryCreated {
        library_id: LibraryId,
        name: String,
    },
    LibraryDeleted {
        library_id: LibraryId,
    },

    // -- Item lifecycle ------------------------------------------------------
    ItemAdded {
        item_id: ItemId,
        item_name: String,
        item_kind: String,
        library_id: LibraryId,
    },
    ItemUpdated {
        item_id: ItemId,
    },
    ItemRemoved {
        item_id: ItemId,
    },

    // -- Conversion ----------------------------------------------------------
    ConversionQueued {
        job_id: ConversionJobId,
    },
    ConversionStarted {
        job_id: ConversionJobId,
    },
    ConversionProgress {
        job_id: ConversionJobId,
        progress: f32,
        encode_fps: Option<f64>,
        eta_secs: Option<f64>,
    },
    ConversionCompleted {
        job_id: ConversionJobId,
    },
    ConversionFailed {
        job_id: ConversionJobId,
        error: String,
    },

    // -- Scan diagnostics ----------------------------------------------------
    LibraryScanError {
        library_id: LibraryId,
        file_path: String,
        message: String,
    },
    ItemEnrichmentQueued {
        item_id: ItemId,
        library_id: LibraryId,
    },
    ItemEnriched {
        item_id: ItemId,
        library_id: LibraryId,
    },
}

// ---------------------------------------------------------------------------
// Event
// ---------------------------------------------------------------------------

/// A timestamped, categorised event ready for broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event identifier.
    pub id: Uuid,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Audience category.
    pub category: EventCategory,
    /// What happened.
    pub payload: EventPayload,
}

impl Event {
    /// Create a new event with a fresh UUID and the current timestamp.
    pub fn new(category: EventCategory, payload: EventPayload) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            category,
            payload,
        }
    }
}

// ---------------------------------------------------------------------------
// EventBus
// ---------------------------------------------------------------------------

/// Broadcast channel with a bounded ring buffer of recent events.
pub struct EventBus {
    tx: broadcast::Sender<Event>,
    recent: RwLock<VecDeque<Event>>,
}

impl EventBus {
    /// Create a new event bus.
    ///
    /// `capacity` controls the broadcast channel buffer size (not the ring
    /// buffer, which is always [`MAX_RECENT_EVENTS`]).
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            recent: RwLock::new(VecDeque::with_capacity(MAX_RECENT_EVENTS)),
        }
    }

    /// Subscribe to the broadcast channel.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    /// Broadcast an event to all current subscribers and store it in the
    /// ring buffer.
    pub fn broadcast(&self, category: EventCategory, payload: EventPayload) {
        let event = Event::new(category, payload);

        // Store in ring buffer regardless of subscriber count.
        {
            let mut recent = self.recent.write();
            if recent.len() >= MAX_RECENT_EVENTS {
                recent.pop_back();
            }
            recent.push_front(event.clone());
        }

        // Ignore send errors (no subscribers).
        let _ = self.tx.send(event);
    }

    /// Return the `n` most recent events (newest first).
    pub fn recent_events(&self, n: usize) -> Vec<Event> {
        let recent = self.recent.read();
        recent.iter().take(n).cloned().collect()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_and_receive() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        let job_id = JobId::new();
        bus.broadcast(EventCategory::Admin, EventPayload::JobQueued { job_id });

        let event = rx.try_recv().unwrap();
        assert_eq!(event.category, EventCategory::Admin);
        match &event.payload {
            EventPayload::JobQueued { job_id: received } => assert_eq!(*received, job_id),
            other => panic!("unexpected payload: {:?}", other),
        }
    }

    #[test]
    fn recent_events_capped() {
        let bus = EventBus::new(256);
        let job_id = JobId::new();

        for _ in 0..150 {
            bus.broadcast(EventCategory::Admin, EventPayload::JobQueued { job_id });
        }

        let recent = bus.recent_events(200);
        assert_eq!(recent.len(), MAX_RECENT_EVENTS);
    }

    #[test]
    fn recent_events_returns_subset() {
        let bus = EventBus::new(16);
        let job_id = JobId::new();

        for _ in 0..10 {
            bus.broadcast(EventCategory::User, EventPayload::ItemAdded { item_id: ItemId::new(), item_name: "Test".into(), item_kind: "movie".into(), library_id: LibraryId::new() });
        }
        bus.broadcast(EventCategory::Admin, EventPayload::JobStarted { job_id });

        let recent = bus.recent_events(3);
        assert_eq!(recent.len(), 3);
        // Most recent first
        assert_eq!(recent[0].category, EventCategory::Admin);
    }

    #[test]
    fn no_subscribers_does_not_panic() {
        let bus = EventBus::new(4);
        bus.broadcast(
            EventCategory::Admin,
            EventPayload::JobFailed {
                job_id: JobId::new(),
                error: "test".into(),
            },
        );
        // Should not panic even without subscribers.
    }

    #[test]
    fn event_serde_roundtrip() {
        let event = Event::new(
            EventCategory::User,
            EventPayload::LibraryCreated {
                library_id: LibraryId::new(),
                name: "Movies".into(),
            },
        );
        let json = serde_json::to_string(&event).unwrap();
        let back: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, event.id);
        assert_eq!(back.category, event.category);
    }

    #[test]
    fn event_payload_variants_serialize() {
        // Ensure all variants can be serialized without error.
        let payloads = vec![
            EventPayload::JobQueued { job_id: JobId::new() },
            EventPayload::JobStarted { job_id: JobId::new() },
            EventPayload::JobProgress { job_id: JobId::new(), progress: 0.5, step: "encoding".into() },
            EventPayload::JobCompleted { job_id: JobId::new() },
            EventPayload::JobFailed { job_id: JobId::new(), error: "err".into() },
            EventPayload::LibraryScanStarted { library_id: LibraryId::new() },
            EventPayload::LibraryScanProgress { library_id: LibraryId::new(), files_found: 10, files_queued: 5, phase: "walking".into(), files_total: 20, files_processed: 10 },
            EventPayload::LibraryScanComplete { library_id: LibraryId::new(), files_found: 100, files_queued: 95, files_skipped: 3, errors: 2 },
            EventPayload::LibraryCreated { library_id: LibraryId::new(), name: "Test".into() },
            EventPayload::LibraryDeleted { library_id: LibraryId::new() },
            EventPayload::ItemAdded { item_id: ItemId::new(), item_name: "Test".into(), item_kind: "movie".into(), library_id: LibraryId::new() },
            EventPayload::ItemUpdated { item_id: ItemId::new() },
            EventPayload::ItemRemoved { item_id: ItemId::new() },
            EventPayload::ConversionQueued { job_id: ConversionJobId::new() },
            EventPayload::ConversionStarted { job_id: ConversionJobId::new() },
            EventPayload::ConversionProgress { job_id: ConversionJobId::new(), progress: 0.75, encode_fps: Some(24.5), eta_secs: Some(120.0) },
            EventPayload::ConversionCompleted { job_id: ConversionJobId::new() },
            EventPayload::ConversionFailed { job_id: ConversionJobId::new(), error: "fail".into() },
            EventPayload::LibraryScanError { library_id: LibraryId::new(), file_path: "/tmp/test.mkv".into(), message: "probe failed".into() },
            EventPayload::ItemEnrichmentQueued { item_id: ItemId::new(), library_id: LibraryId::new() },
            EventPayload::ItemEnriched { item_id: ItemId::new(), library_id: LibraryId::new() },
        ];
        for p in &payloads {
            let json = serde_json::to_string(p).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[test]
    fn default_event_bus() {
        let bus = EventBus::default();
        assert!(bus.recent_events(10).is_empty());
    }
}
