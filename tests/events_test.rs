//! Integration tests for SSE events endpoint.

mod common;

use common::TestHarness;

#[tokio::test]
async fn sse_stream_connects() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{addr}/api/events"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify content type is event-stream.
    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.contains("text/event-stream"), "expected SSE content-type, got: {ct}");
}

#[tokio::test]
async fn sse_with_category_filter() {
    let (_h, addr) = TestHarness::with_server().await;
    let client = reqwest::Client::new();

    // Request with admin category filter.
    let resp = client
        .get(format!("http://{addr}/api/events?category=admin"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}
