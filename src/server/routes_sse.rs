use crate::server::AppContext;
use crate::state::AppEvent;
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use futures::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

pub fn sse_routes() -> Router<AppContext> {
    Router::new().route("/events", get(events_handler))
}

pub async fn events_handler(
    State(ctx): State<AppContext>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = ctx.state.subscribe();

    let stream = BroadcastStream::new(rx)
        .filter_map(|result| result.ok())
        .map(|event: AppEvent| {
            // Get the event type string for the SSE event field
            let event_type = match &event {
                // Job events (admin)
                AppEvent::JobQueued { .. } => "job:queued",
                AppEvent::JobStarted { .. } => "job:started",
                AppEvent::JobProgress { .. } => "job:progress",
                AppEvent::JobCompleted { .. } => "job:completed",
                AppEvent::JobFailed { .. } => "job:failed",
                // Library events (user)
                AppEvent::LibraryScanStarted { .. } => "library:scan_started",
                AppEvent::LibraryScanComplete { .. } => "library:scan_complete",
                AppEvent::LibraryCreated { .. } => "library:created",
                AppEvent::LibraryDeleted { .. } => "library:deleted",
                // Item events (user)
                AppEvent::ItemAdded { .. } => "item:added",
                AppEvent::ItemUpdated { .. } => "item:updated",
                AppEvent::ItemRemoved { .. } => "item:removed",
                AppEvent::PlaybackAvailable { .. } => "item:playback_available",
            };

            // Serialize the entire event as JSON (includes category field)
            let data = serde_json::to_string(&event).unwrap_or_else(|e| {
                format!(r#"{{"error": "serialization failed: {}"}}"#, e)
            });

            Ok(Event::default().event(event_type).data(data))
        });

    // Add keepalive with heartbeat every 30 seconds
    let heartbeat =
        tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(30)))
            .map(|_| {
                Ok(Event::default()
                    .event("heartbeat")
                    .data(r#"{"event_type":"heartbeat","category":"user"}"#))
            });

    let combined = stream.merge(heartbeat);

    Sse::new(combined).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
