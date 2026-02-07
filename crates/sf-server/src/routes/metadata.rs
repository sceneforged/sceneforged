//! Metadata enrichment routes (TMDB search + enrich).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::context::AppContext;
use crate::error::AppError;
use crate::tmdb::TmdbClient;

// ---------------------------------------------------------------------------
// TMDB search proxy
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub year: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub tmdb_id: u64,
    pub title: Option<String>,
    pub year: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
}

/// GET /api/tmdb/search — proxy search to TMDB.
pub async fn tmdb_search(
    State(ctx): State<AppContext>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    let client = build_tmdb_client(&ctx)?;

    let media_type = params.media_type.as_deref().unwrap_or("movie");
    let results = match media_type {
        "tv" => client.search_tv(&params.q, params.year).await?,
        _ => client.search_movie(&params.q, params.year).await?,
    };

    let results: Vec<SearchResult> = results
        .into_iter()
        .take(10)
        .map(|r| SearchResult {
            tmdb_id: r.id,
            title: r.title,
            year: r.release_date.as_ref().and_then(|d| d.get(..4).map(String::from)),
            overview: r.overview,
            poster_path: r.poster_path,
        })
        .collect();

    Ok(Json(SearchResponse { results }))
}

// ---------------------------------------------------------------------------
// Enrich item
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct EnrichRequest {
    /// TMDB ID to enrich from. If omitted, auto-search by item name.
    pub tmdb_id: Option<u64>,
    /// "movie" or "tv" — defaults to item's kind.
    #[serde(rename = "type")]
    pub media_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EnrichResponse {
    pub updated: bool,
    pub tmdb_id: Option<u64>,
    pub images_downloaded: u32,
}

/// POST /api/items/{id}/enrich — trigger TMDB lookup and apply metadata.
pub async fn enrich_item(
    State(ctx): State<AppContext>,
    Path(item_id): Path<String>,
) -> Result<(StatusCode, Json<EnrichResponse>), AppError> {
    enrich_item_with_body(ctx, item_id, EnrichRequest { tmdb_id: None, media_type: None }).await
}

/// POST /api/items/{id}/enrich with optional body.
pub async fn enrich_item_with_body(
    ctx: AppContext,
    item_id: String,
    body: EnrichRequest,
) -> Result<(StatusCode, Json<EnrichResponse>), AppError> {
    let id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let item = sf_db::queries::items::get_item(&conn, id)?
        .ok_or_else(|| sf_core::Error::not_found("item", id))?;
    drop(conn);

    let client = build_tmdb_client(&ctx)?;

    let is_tv = body.media_type.as_deref() == Some("tv")
        || item.item_kind == "series"
        || item.item_kind == "episode";

    // Resolve TMDB ID (provided or auto-search).
    let tmdb_id = if let Some(id) = body.tmdb_id {
        id
    } else {
        let results = if is_tv {
            client.search_tv(&item.name, item.year.map(|y| y as u32)).await?
        } else {
            client.search_movie(&item.name, item.year.map(|y| y as u32)).await?
        };
        results
            .first()
            .map(|r| r.id)
            .ok_or_else(|| sf_core::Error::not_found("tmdb_result", &item.name))?
    };

    let mut images_downloaded: u32 = 0;

    if is_tv {
        let show = client.get_tv(tmdb_id).await?;

        let conn = sf_db::pool::get_conn(&ctx.db)?;
        let provider_ids = serde_json::json!({ "tmdb": tmdb_id }).to_string();
        sf_db::queries::items::update_item(
            &conn,
            id,
            &item.name,
            item.sort_name.as_deref(),
            item.year,
            show.overview.as_deref(),
            None,
            show.vote_average,
            Some(&provider_ids),
            item.parent_id,
            item.season_number,
            item.episode_number,
        )?;
        drop(conn);

        images_downloaded += download_and_store_images(&ctx, &client, id, show.poster_path.as_deref(), show.backdrop_path.as_deref()).await;
    } else {
        let movie = client.get_movie(tmdb_id).await?;

        let conn = sf_db::pool::get_conn(&ctx.db)?;
        let mut pids = serde_json::json!({ "tmdb": tmdb_id });
        if let Some(ref imdb) = movie.imdb_id {
            pids["imdb"] = serde_json::Value::String(imdb.clone());
        }
        sf_db::queries::items::update_item(
            &conn,
            id,
            &item.name,
            item.sort_name.as_deref(),
            item.year,
            movie.overview.as_deref(),
            movie.runtime,
            movie.vote_average,
            Some(&pids.to_string()),
            item.parent_id,
            item.season_number,
            item.episode_number,
        )?;
        drop(conn);

        images_downloaded += download_and_store_images(&ctx, &client, id, movie.poster_path.as_deref(), movie.backdrop_path.as_deref()).await;
    }

    Ok((StatusCode::OK, Json(EnrichResponse {
        updated: true,
        tmdb_id: Some(tmdb_id),
        images_downloaded,
    })))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_tmdb_client(ctx: &AppContext) -> sf_core::Result<TmdbClient> {
    let meta = ctx.config_store.metadata.read();
    let api_key = meta.tmdb_api_key.clone()
        .ok_or_else(|| sf_core::Error::Validation("TMDB API key not configured".into()))?;
    let language = meta.language.clone();
    drop(meta);
    Ok(TmdbClient::new(api_key, language))
}

async fn download_and_store_images(
    ctx: &AppContext,
    client: &TmdbClient,
    item_id: sf_core::ItemId,
    poster_path: Option<&str>,
    backdrop_path: Option<&str>,
) -> u32 {
    let storage_dir = ctx.config_store.images.read().storage_dir.clone();
    let item_dir = storage_dir.join(item_id.to_string());
    if let Err(e) = std::fs::create_dir_all(&item_dir) {
        tracing::warn!(error = %e, "Failed to create image directory");
        return 0;
    }

    let mut count = 0u32;

    if let Some(path) = poster_path {
        match client.download_image(path, "w500").await {
            Ok(bytes) => {
                let file_path = item_dir.join("primary.jpg");
                if std::fs::write(&file_path, &bytes).is_ok() {
                    let conn = sf_db::pool::get_conn(&ctx.db).ok();
                    if let Some(conn) = conn {
                        let _ = sf_db::queries::images::create_image(
                            &conn,
                            item_id,
                            "primary",
                            &file_path.to_string_lossy(),
                            Some("tmdb"),
                            Some(500),
                            None,
                        );
                    }
                    count += 1;
                }
            }
            Err(e) => tracing::warn!(error = %e, "Failed to download poster"),
        }
    }

    if let Some(path) = backdrop_path {
        match client.download_image(path, "w1280").await {
            Ok(bytes) => {
                let file_path = item_dir.join("backdrop.jpg");
                if std::fs::write(&file_path, &bytes).is_ok() {
                    let conn = sf_db::pool::get_conn(&ctx.db).ok();
                    if let Some(conn) = conn {
                        let _ = sf_db::queries::images::create_image(
                            &conn,
                            item_id,
                            "backdrop",
                            &file_path.to_string_lossy(),
                            Some("tmdb"),
                            Some(1280),
                            None,
                        );
                    }
                    count += 1;
                }
            }
            Err(e) => tracing::warn!(error = %e, "Failed to download backdrop"),
        }
    }

    count
}
