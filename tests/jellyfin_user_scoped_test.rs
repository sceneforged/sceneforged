//! Integration tests for Jellyfin user-scoped route aliases.
//!
//! Infuse and other clients use `/Users/{uid}/Views`, `/Users/{uid}/Items`
//! instead of the top-level equivalents. These should behave identically.

mod common;

use common::TestHarness;

#[tokio::test]
async fn user_scoped_views() {
    let (h, addr) = TestHarness::with_server().await;
    h.create_library_named("Movies", "movies");
    h.create_library_named("TV Shows", "tvshows");

    // Any user_id works — the route ignores the {user_id} path param.
    let resp = reqwest::get(format!(
        "http://{addr}/Users/00000000-0000-0000-0000-000000000000/Views"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let items = json["Items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(json["TotalRecordCount"], 2);
}

#[tokio::test]
async fn user_scoped_items() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, lib_id_str) = h.create_library();
    h.create_item_with_media(lib_id, "Movie A", "movie");
    h.create_item_with_media(lib_id, "Movie B", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/Users/00000000-0000-0000-0000-000000000000/Items?ParentId={lib_id_str}"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let items = json["Items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn user_scoped_get_item() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "Scoped Item", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/Users/00000000-0000-0000-0000-000000000000/Items/{item_id_str}"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["Name"], "Scoped Item");
    assert_eq!(json["Id"], item_id_str);
}
