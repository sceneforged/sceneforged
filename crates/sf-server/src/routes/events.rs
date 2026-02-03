//! Server-Sent Events (SSE) handler.
//!
//! Subscribes to the [`sf_core::events::EventBus`], optionally filters by category, replays
//! recent events for late joiners, and sends keepalive heartbeats.

use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use serde::Deserialize;
use std::convert::Infallible;
use std::time::Duration;

use crate::context::AppContext;

/// Optional query parameter for category filtering.
#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    /// Filter events by category: "admin" or "user".
    pub category: Option<String>,
}

/// GET /api/events -- SSE stream of application events.
pub async fn events_handler(
    State(ctx): State<AppContext>,
    Query(params): Query<EventsQuery>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>> {
    let category_filter = params.category.clone();

    // Replay recent events for late joiners.
    let recent = ctx.event_bus.recent_events(50);
    let mut rx = ctx.event_bus.subscribe();

    let stream = async_stream::stream! {
        // Send recent events first.
        for event in recent.into_iter().rev() {
            if matches_category(&event.category, &category_filter) {
                if let Ok(data) = serde_json::to_string(&event) {
                    yield Ok(Event::default().data(data));
                }
            }
        }

        // Heartbeat interval.
        let mut heartbeat = tokio::time::interval(Duration::from_secs(15));

        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            if matches_category(&event.category, &category_filter) {
                                if let Ok(data) = serde_json::to_string(&event) {
                                    yield Ok(Event::default().data(data));
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::debug!("SSE client lagged by {n} events");
                            // Continue receiving.
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    yield Ok(Event::default()
                        .event("heartbeat")
                        .data(r#"{"type":"heartbeat"}"#));
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

fn matches_category(
    event_category: &sf_core::events::EventCategory,
    filter: &Option<String>,
) -> bool {
    let Some(ref f) = filter else {
        return true;
    };
    match f.as_str() {
        "admin" => *event_category == sf_core::events::EventCategory::Admin,
        "user" => *event_category == sf_core::events::EventCategory::User,
        _ => true,
    }
}
