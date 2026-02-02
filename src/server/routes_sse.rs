use crate::server::AppContext;
use crate::state::JobEvent;
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
        .map(|event: JobEvent| {
            let (event_type, data) = match &event {
                JobEvent::Queued(job) => (
                    "job:queued",
                    serde_json::json!({
                        "type": "queued",
                        "job": job
                    }),
                ),
                JobEvent::Started { id, rule_name } => (
                    "job:started",
                    serde_json::json!({
                        "type": "started",
                        "id": id,
                        "rule_name": rule_name
                    }),
                ),
                JobEvent::Progress { id, progress, step } => (
                    "job:progress",
                    serde_json::json!({
                        "type": "progress",
                        "id": id,
                        "progress": progress,
                        "step": step
                    }),
                ),
                JobEvent::Completed(job) => (
                    "job:completed",
                    serde_json::json!({
                        "type": "completed",
                        "job": job
                    }),
                ),
                JobEvent::Failed { id, error } => (
                    "job:failed",
                    serde_json::json!({
                        "type": "failed",
                        "id": id,
                        "error": error
                    }),
                ),
            };

            Ok(Event::default().event(event_type).data(data.to_string()))
        });

    // Add keepalive with heartbeat every 30 seconds
    let heartbeat =
        tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(30)))
            .map(|_| {
                Ok(Event::default()
                    .event("heartbeat")
                    .data(r#"{"type":"heartbeat"}"#))
            });

    let combined = stream.merge(heartbeat);

    Sse::new(combined).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
