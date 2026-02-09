//! Integration tests for item query routes.

mod common;

use common::TestHarness;

#[tokio::test]
async fn list_items_empty_without_library_id() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!("http://{addr}/api/items"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let items: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn list_items_by_library() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, lib_id_str) = h.create_library();
    h.create_item(lib_id);
    h.create_item_with_media(lib_id, "Another Movie", "movie");

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/items?library_id={lib_id_str}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let items: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(items.len(), 2);
}

#[tokio::test]
async fn get_item_with_media_files() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item_id_str) = h.create_item(lib_id);

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{addr}/api/items/{item_id_str}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["name"], "Test Movie");
    assert_eq!(json["year"], 2024);
    assert!(json["media_files"].is_array());
    assert_eq!(json["media_files"].as_array().unwrap().len(), 1);
    assert!(json["images"].is_array());
}

#[tokio::test]
async fn get_item_not_found() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!(
        "http://{addr}/api/items/00000000-0000-0000-0000-000000000001"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn list_item_files() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, item_id_str) = h.create_item(lib_id);

    let resp = reqwest::get(format!("http://{addr}/api/items/{item_id_str}/files"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let files: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["file_name"], "test.mkv");
    assert_eq!(files[0]["profile"], "C");
}

#[tokio::test]
async fn list_children() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (series_id, _season_id, episode_ids) =
        h.create_series_hierarchy(lib_id, "Breaking Bad", 3);

    // Children of the series should be seasons.
    let resp = reqwest::get(format!(
        "http://{addr}/api/items/{}/children",
        series_id
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let children: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["item_kind"], "season");

    // Children of the season should be episodes.
    let resp = reqwest::get(format!(
        "http://{addr}/api/items/{}/children",
        _season_id
    ))
    .await
    .unwrap();
    let episodes: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(episodes.len(), 3);
    assert_eq!(episodes[0]["item_kind"], "episode");
    let _ = episode_ids;
}

#[tokio::test]
async fn search_items_fts() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    h.create_item_with_media(lib_id, "The Matrix", "movie");
    h.create_item_with_media(lib_id, "Inception", "movie");

    let resp = reqwest::get(format!("http://{addr}/api/search?q=Matrix"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let results: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["name"], "The Matrix");
}

#[tokio::test]
async fn search_items_empty_query() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!("http://{addr}/api/search?q="))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let results: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn search_with_library_filter() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib1, lib1_str) = h.create_library_named("Movies", "movies");
    let (lib2, _) = h.create_library_named("TV", "tvshows");
    h.create_item_with_media(lib1, "The Matrix", "movie");
    h.create_item_with_media(lib2, "The Matrix Show", "series");

    // Both items match "Matrix", but library filter narrows to lib1.
    let resp = reqwest::get(format!(
        "http://{addr}/api/search?q=Matrix&library_id={lib1_str}"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let results: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["name"], "The Matrix");
}

#[tokio::test]
async fn search_with_item_kind_filter() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    h.create_item_with_media(lib_id, "Star Wars", "movie");
    h.create_item_with_media(lib_id, "Star Trek Show", "series");

    // Both match "Star", but item_kind filter narrows to movies.
    let resp = reqwest::get(format!(
        "http://{addr}/api/search?q=Star&item_kind=movie"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let results: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["name"], "Star Wars");
}

#[tokio::test]
async fn list_items_with_pagination() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, lib_id_str) = h.create_library();
    for i in 0..10 {
        h.create_item_with_media(lib_id, &format!("Movie {i:02}"), "movie");
    }

    let resp = reqwest::get(format!(
        "http://{addr}/api/items?library_id={lib_id_str}&offset=3&limit=3"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let items: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(items.len(), 3);
}

#[tokio::test]
async fn list_items_with_search_param() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    h.create_item_with_media(lib_id, "The Matrix", "movie");
    h.create_item_with_media(lib_id, "Inception", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/api/items?search=Matrix"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let items: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "The Matrix");
}
