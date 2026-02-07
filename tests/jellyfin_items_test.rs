//! Integration tests for Jellyfin items/library browsing endpoints.

mod common;

use common::TestHarness;

#[tokio::test]
async fn user_views() {
    let (h, addr) = TestHarness::with_server().await;
    h.create_library_named("Movies", "movies");
    h.create_library_named("TV Shows", "tvshows");

    let resp = reqwest::get(format!("http://{addr}/UserViews"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let items = json["Items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(json["TotalRecordCount"], 2);

    // Each should be a CollectionFolder.
    for item in items {
        assert_eq!(item["Type"], "CollectionFolder");
        assert!(item["CollectionType"].is_string());
    }
}

#[tokio::test]
async fn list_items_by_parent_id() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, lib_id_str) = h.create_library();
    h.create_item_with_media(lib_id, "Movie A", "movie");
    h.create_item_with_media(lib_id, "Movie B", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/Items?ParentId={lib_id_str}"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let items = json["Items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    // Each should be a Movie type.
    for item in items {
        assert_eq!(item["Type"], "Movie");
    }
}

#[tokio::test]
async fn get_item_by_id() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "JF Item", "movie");

    let resp = reqwest::get(format!("http://{addr}/Items/{item_id_str}"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["Name"], "JF Item");
    assert_eq!(json["Type"], "Movie");
    assert_eq!(json["Id"], item_id_str);
    // Playable items should have MediaSources.
    assert!(json["MediaSources"].is_array());
}

#[tokio::test]
async fn get_item_not_found() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!(
        "http://{addr}/Items/00000000-0000-0000-0000-000000000001"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn show_seasons() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library_named("TV", "tvshows");
    let (series_id, _season_id, _eps) =
        h.create_series_hierarchy(lib_id, "Test Show", 3);

    let resp = reqwest::get(format!(
        "http://{addr}/Shows/{}/Seasons",
        series_id
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let items = json["Items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["Type"], "Season");
    assert_eq!(items[0]["SeriesId"], series_id.to_string());
}

#[tokio::test]
async fn show_episodes_by_season() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library_named("TV2", "tvshows");
    let (series_id, season_id, _eps) =
        h.create_series_hierarchy(lib_id, "Test Show 2", 5);

    let resp = reqwest::get(format!(
        "http://{addr}/Shows/{}/Episodes?SeasonId={}",
        series_id, season_id
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let items = json["Items"].as_array().unwrap();
    assert_eq!(items.len(), 5);
    assert_eq!(items[0]["Type"], "Episode");
}

#[tokio::test]
async fn show_episodes_all_seasons() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library_named("TV3", "tvshows");
    let (series_id, _, _eps) =
        h.create_series_hierarchy(lib_id, "Test Show 3", 4);

    let resp = reqwest::get(format!(
        "http://{addr}/Shows/{}/Episodes",
        series_id
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let items = json["Items"].as_array().unwrap();
    assert_eq!(items.len(), 4);
}

#[tokio::test]
async fn search_hints() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    h.create_item_with_media(lib_id, "The Matrix", "movie");
    h.create_item_with_media(lib_id, "Inception", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/Search/Hints?searchTerm=Matrix"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    let hints = json["SearchHints"].as_array().unwrap();
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0]["Name"], "The Matrix");
    assert_eq!(hints[0]["Type"], "Movie");
    assert_eq!(json["TotalRecordCount"], 1);
}

#[tokio::test]
async fn search_hints_empty() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!("http://{addr}/Search/Hints?searchTerm="))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["SearchHints"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn next_up_stub() {
    let (_h, addr) = TestHarness::with_server().await;
    let resp = reqwest::get(format!("http://{addr}/Shows/NextUp"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert!(json["Items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn list_items_with_search_term() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    h.create_item_with_media(lib_id, "Star Wars", "movie");
    h.create_item_with_media(lib_id, "Star Trek", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/Items?SearchTerm=Star"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(json["Items"].as_array().unwrap().len(), 2);
}
