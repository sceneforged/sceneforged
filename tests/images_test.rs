//! Integration tests for image serving routes.

mod common;

use common::TestHarness;

#[tokio::test]
async fn get_image_serves_jpeg() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (item_id, _, item_id_str, _) = h.create_item_with_media(lib_id, "ImageMovie", "movie");

    // Write a fake JPEG to a temp file.
    let dir = tempfile::tempdir().unwrap();
    let img_path = dir.path().join("primary.jpg");
    std::fs::write(&img_path, b"\xFF\xD8\xFF fake jpeg data").unwrap();

    // Insert an image record pointing to our temp file.
    let conn = h.conn();
    sf_db::queries::images::create_image(
        &conn,
        item_id,
        "primary",
        img_path.to_str().unwrap(),
        None,
        Some(400),
        Some(600),
    )
    .unwrap();

    let resp = reqwest::get(format!(
        "http://{addr}/api/images/{item_id_str}/primary/original"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "image/jpeg"
    );
    let body = resp.bytes().await.unwrap();
    assert!(body.starts_with(b"\xFF\xD8\xFF"));
}

#[tokio::test]
async fn get_image_png_content_type() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (item_id, _, item_id_str, _) = h.create_item_with_media(lib_id, "PngMovie", "movie");

    let dir = tempfile::tempdir().unwrap();
    let img_path = dir.path().join("backdrop.png");
    std::fs::write(&img_path, b"\x89PNG fake png data").unwrap();

    let conn = h.conn();
    sf_db::queries::images::create_image(
        &conn,
        item_id,
        "backdrop",
        img_path.to_str().unwrap(),
        None,
        Some(1920),
        Some(1080),
    )
    .unwrap();

    let resp = reqwest::get(format!(
        "http://{addr}/api/images/{item_id_str}/backdrop/original"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "image/png"
    );
}

#[tokio::test]
async fn get_image_not_found() {
    let (h, addr) = TestHarness::with_server().await;
    let (lib_id, _) = h.create_library();
    let (_, _, item_id_str, _) = h.create_item_with_media(lib_id, "NoImg", "movie");

    let resp = reqwest::get(format!(
        "http://{addr}/api/images/{item_id_str}/primary/original"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn get_image_invalid_item_id() {
    let (_h, addr) = TestHarness::with_server().await;

    let resp = reqwest::get(format!(
        "http://{addr}/api/images/not-a-uuid/primary/original"
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 400);
}
