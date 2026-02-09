//! Jellyfin items/library browsing endpoints.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use rand::seq::SliceRandom;
use serde::Deserialize;

use crate::context::AppContext;
use crate::error::AppError;
use crate::middleware::auth::validate_auth_headers;

use super::dto::{self, BaseItemDto, ItemsResult, SearchHint, SearchHintResult};

/// Build a BaseItemDto for a library (CollectionFolder).
fn library_to_dto(lib: &sf_db::models::Library, child_count: i32) -> BaseItemDto {
    BaseItemDto {
        id: lib.id.to_string(),
        name: lib.name.clone(),
        item_type: "CollectionFolder".to_string(),
        collection_type: Some(lib.media_type.clone()),
        overview: None,
        production_year: None,
        run_time_ticks: None,
        community_rating: None,
        parent_id: None,
        series_id: None,
        series_name: None,
        season_id: None,
        index_number: None,
        parent_index_number: None,
        image_tags: None,
        user_data: None,
        media_sources: None,
        media_streams: None,
        media_type: None,
        location_type: Some("FileSystem".to_string()),
        video_type: None,
        child_count: Some(child_count),
        recursive_item_count: Some(child_count),
        date_created: None,
        etag: Some(lib.id.to_string().get(..8).unwrap_or("00000000").to_string()),
        sort_name: Some(lib.name.clone()),
        path: None,
        provider_ids: Some(std::collections::HashMap::new()),
        genres: Some(Vec::new()),
    }
}

/// Case-insensitive query params (Jellyfin clients send both camelCase and PascalCase).
#[derive(Debug, Deserialize)]
pub struct ItemsQuery {
    #[serde(alias = "parentId", alias = "ParentId")]
    pub parent_id: Option<String>,
    #[serde(alias = "includeItemTypes", alias = "IncludeItemTypes")]
    pub include_item_types: Option<String>,
    #[serde(alias = "startIndex", alias = "StartIndex")]
    pub start_index: Option<i64>,
    #[serde(alias = "limit", alias = "Limit")]
    pub limit: Option<i64>,
    #[serde(alias = "sortBy", alias = "SortBy")]
    pub sort_by: Option<String>,
    #[serde(alias = "sortOrder", alias = "SortOrder")]
    pub sort_order: Option<String>,
    #[serde(alias = "searchTerm", alias = "SearchTerm")]
    pub search_term: Option<String>,
    #[serde(alias = "userId", alias = "UserId")]
    pub user_id: Option<String>,
    #[serde(alias = "seriesId", alias = "SeriesId")]
    pub series_id: Option<String>,
    #[serde(alias = "seasonId", alias = "SeasonId")]
    pub season_id: Option<String>,
    #[serde(alias = "recursive", alias = "Recursive")]
    pub recursive: Option<bool>,
    #[serde(alias = "isFavorite", alias = "IsFavorite")]
    pub is_favorite: Option<bool>,
    #[serde(alias = "filters", alias = "Filters")]
    pub filters: Option<String>,
}

/// Map Jellyfin type names to our internal item_kind values.
fn jellyfin_type_to_kind(jf_type: &str) -> Option<&'static str> {
    match jf_type.trim() {
        "Movie" => Some("movie"),
        "Series" => Some("series"),
        "Season" => Some("season"),
        "Episode" => Some("episode"),
        _ => None,
    }
}

/// Collect all descendants of a parent item recursively (max 3 levels).
fn collect_descendants(
    conn: &rusqlite::Connection,
    parent_id: sf_core::ItemId,
    depth: u8,
) -> Vec<sf_db::models::Item> {
    if depth > 3 {
        return Vec::new();
    }
    let children = sf_db::queries::items::list_children(conn, parent_id).unwrap_or_default();
    let mut result = Vec::new();
    for child in children {
        let child_id = child.id;
        result.push(child);
        result.extend(collect_descendants(conn, child_id, depth + 1));
    }
    result
}

/// Apply includeItemTypes filtering to a list of items.
fn filter_by_types(items: Vec<sf_db::models::Item>, include_types: &str) -> Vec<sf_db::models::Item> {
    let kinds: Vec<&str> = include_types
        .split(',')
        .filter_map(jellyfin_type_to_kind)
        .collect();
    if kinds.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| kinds.contains(&item.item_kind.as_str()))
        .collect()
}

/// Apply sortBy/sortOrder to items.
fn sort_items(items: &mut [sf_db::models::Item], sort_by: &str, sort_order: &str) {
    let descending = sort_order.eq_ignore_ascii_case("Descending");
    match sort_by.split(',').next().unwrap_or("") {
        "SortName" => {
            items.sort_by(|a, b| {
                let a_name = a.sort_name.as_deref().unwrap_or(&a.name);
                let b_name = b.sort_name.as_deref().unwrap_or(&b.name);
                let cmp = a_name.to_lowercase().cmp(&b_name.to_lowercase());
                if descending { cmp.reverse() } else { cmp }
            });
        }
        "DateCreated" => {
            items.sort_by(|a, b| {
                let cmp = a.created_at.cmp(&b.created_at);
                if descending { cmp.reverse() } else { cmp }
            });
        }
        "CommunityRating" => {
            items.sort_by(|a, b| {
                let ar = a.community_rating.unwrap_or(0.0);
                let br = b.community_rating.unwrap_or(0.0);
                let cmp = ar.partial_cmp(&br).unwrap_or(std::cmp::Ordering::Equal);
                if descending { cmp.reverse() } else { cmp }
            });
        }
        "Random" => {
            let mut rng = rand::thread_rng();
            items.shuffle(&mut rng);
        }
        _ => {}
    }
}

/// GET /UserViews -- list top-level library views.
pub async fn user_views(
    State(ctx): State<AppContext>,
) -> Result<Json<ItemsResult>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let libraries = sf_db::queries::libraries::list_libraries(&conn)?;

    let items: Vec<BaseItemDto> = libraries
        .iter()
        .map(|lib| {
            let count = sf_db::queries::items::count_items_by_library(&conn, lib.id)
                .unwrap_or(0) as i32;
            library_to_dto(lib, count)
        })
        .collect();

    let count = items.len();
    Ok(Json(ItemsResult {
        items,
        total_record_count: count,
    }))
}

/// GET /Items -- list items with filtering.
pub async fn list_items(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(params): Query<ItemsQuery>,
) -> Result<Json<ItemsResult>, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);
    let conn = sf_db::pool::get_conn(&ctx.db)?;

    let offset = params.start_index.unwrap_or(0);
    let limit = params.limit.unwrap_or(50).min(200);

    // Check for isFavorite filter.
    if params.is_favorite == Some(true) {
        let kind_strs: Vec<&str> = params
            .include_item_types
            .as_deref()
            .map(|s| s.split(',').filter_map(jellyfin_type_to_kind).collect())
            .unwrap_or_default();
        let kinds = if kind_strs.is_empty() { None } else { Some(kind_strs.as_slice()) };
        let lib_id = params.parent_id.as_deref().and_then(|s| s.parse().ok());
        let mut items = sf_db::queries::items::list_favorite_items(
            &conn, user_id, lib_id, kinds, offset, limit,
        )?;

        if let Some(ref sort_by) = params.sort_by {
            sort_items(&mut items, sort_by, params.sort_order.as_deref().unwrap_or("Ascending"));
        }

        return build_response(&conn, user_id, &items);
    }

    // Check for IsResumable filter.
    if let Some(ref filters) = params.filters {
        if filters.contains("IsResumable") {
            let kind_strs: Vec<&str> = params
                .include_item_types
                .as_deref()
                .map(|s| s.split(',').filter_map(jellyfin_type_to_kind).collect())
                .unwrap_or_default();
            let kinds = if kind_strs.is_empty() { None } else { Some(kind_strs.as_slice()) };
            let lib_id = params.parent_id.as_deref().and_then(|s| s.parse().ok());
            let mut items = sf_db::queries::items::list_resumable_items(
                &conn, user_id, lib_id, kinds, offset, limit,
            )?;

            if let Some(ref sort_by) = params.sort_by {
                sort_items(&mut items, sort_by, params.sort_order.as_deref().unwrap_or("Ascending"));
            }

            return build_response(&conn, user_id, &items);
        }
    }

    // Standard item listing.
    let mut items = if let Some(ref parent_id) = params.parent_id {
        let recursive = params.recursive.unwrap_or(false);
        // Check if parent is a library or an item.
        if let Ok(lib_id) = parent_id.parse::<sf_core::LibraryId>() {
            if sf_db::queries::libraries::get_library(&conn, lib_id)?.is_some() {
                if recursive {
                    // Return all items in this library.
                    sf_db::queries::items::list_items_by_library(&conn, lib_id, offset, limit)?
                } else {
                    // List top-level items (movies + series, no episodes/seasons directly).
                    sf_db::queries::items::list_items_by_library(&conn, lib_id, offset, limit)?
                        .into_iter()
                        .filter(|i| i.item_kind == "movie" || i.item_kind == "series")
                        .collect()
                }
            } else if let Ok(item_id) = parent_id.parse::<sf_core::ItemId>() {
                if recursive {
                    collect_descendants(&conn, item_id, 0)
                } else {
                    sf_db::queries::items::list_children(&conn, item_id)?
                }
            } else {
                Vec::new()
            }
        } else if let Ok(item_id) = parent_id.parse::<sf_core::ItemId>() {
            if recursive {
                collect_descendants(&conn, item_id, 0)
            } else {
                sf_db::queries::items::list_children(&conn, item_id)?
            }
        } else {
            Vec::new()
        }
    } else if let Some(ref search) = params.search_term {
        sf_db::queries::items::search_items(&conn, search, limit)?
    } else {
        // Return all items without a parent_id filter -- just first page.
        let libraries = sf_db::queries::libraries::list_libraries(&conn)?;
        let mut all = Vec::new();
        for lib in &libraries {
            let lib_items = sf_db::queries::items::list_items_by_library(&conn, lib.id, offset, limit)?;
            all.extend(lib_items);
        }
        all
    };

    // Apply includeItemTypes filter.
    if let Some(ref types) = params.include_item_types {
        items = filter_by_types(items, types);
    }

    // Apply sorting.
    if let Some(ref sort_by) = params.sort_by {
        sort_items(&mut items, sort_by, params.sort_order.as_deref().unwrap_or("Ascending"));
    }

    build_response(&conn, user_id, &items)
}

/// Filter out items that are still being scanned or failed to probe.
/// Jellyfin clients should only see fully-ready items.
fn filter_ready_items(items: &[sf_db::models::Item]) -> Vec<&sf_db::models::Item> {
    items
        .iter()
        .filter(|i| i.scan_status.is_none())
        .collect()
}

/// Build the ItemsResult response with user data for a list of items.
fn build_response(
    conn: &rusqlite::Connection,
    user_id: sf_core::UserId,
    items: &[sf_db::models::Item],
) -> Result<Json<ItemsResult>, AppError> {
    let ready = filter_ready_items(items);
    let item_ids: Vec<sf_core::ItemId> = ready.iter().map(|i| i.id).collect();
    let user_data_map = sf_db::queries::playback::batch_get_user_data(conn, user_id, &item_ids)?;
    let mut images_map = sf_db::queries::images::batch_get_images(conn, &item_ids)?;

    let dtos: Vec<BaseItemDto> = ready
        .iter()
        .map(|item| {
            let images = images_map.remove(&item.id).unwrap_or_default();
            let ud = user_data_map.get(&item.id);
            dto::item_to_dto(item, &images, ud)
        })
        .collect();

    let count = dtos.len();
    Ok(Json(ItemsResult {
        items: dtos,
        total_record_count: count,
    }))
}

/// GET /Items/{id} -- get a single item.
pub async fn get_item(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<BaseItemDto>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // Infuse fetches library views by their ID via this endpoint.
    // Check if the ID is a library first, then fall back to item lookup.
    if let Ok(lib_id) = id.parse::<sf_core::LibraryId>() {
        if let Some(lib) = sf_db::queries::libraries::get_library(&conn, lib_id)? {
            let count = sf_db::queries::items::count_items_by_library(&conn, lib_id)
                .unwrap_or(0) as i32;
            return Ok(Json(library_to_dto(&lib, count)));
        }
    }

    let item_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item_id".into()))?;

    let user_id = resolve_user_from_headers(&ctx, &headers);
    let item = sf_db::queries::items::get_item(&conn, item_id)?
        .ok_or_else(|| sf_core::Error::not_found("item", item_id))?;

    let images = sf_db::queries::images::list_images_by_item(&conn, item_id)
        .unwrap_or_default();
    let user_data_map =
        sf_db::queries::playback::batch_get_user_data(&conn, user_id, &[item_id])?;
    let ud = user_data_map.get(&item_id);
    let mut item_dto = dto::item_to_dto(&item, &images, ud);

    // Add media sources for playable items (with MediaStreams for codec info).
    if item.item_kind == "movie" || item.item_kind == "episode" {
        let media_files = sf_db::queries::media_files::list_media_files_by_item(&conn, item_id)?;
        let sources: Vec<dto::MediaSourceDto> = media_files
            .iter()
            .map(|mf| {
                let ticks = mf.duration_secs.map(|d| (d * dto::TICKS_PER_SECOND as f64) as i64);
                let direct_stream_url = format!(
                    "/Videos/{}/stream?mediaSourceId={}&static=true",
                    item_id, mf.id,
                );

                // Build media streams so clients know codec info before PlaybackInfo.
                let mut streams = Vec::new();
                let mut idx = 0i32;

                if let Some(ref codec) = mf.video_codec {
                    let display = match (mf.resolution_width, mf.resolution_height) {
                        (Some(w), Some(h)) => format!("{w}x{h} {codec}"),
                        _ => codec.clone(),
                    };
                    streams.push(dto::MediaStreamDto {
                        stream_type: "Video".to_string(),
                        index: idx,
                        codec: Some(codec.clone()),
                        language: None,
                        display_title: Some(display),
                        is_default: true,
                        is_forced: false,
                        width: mf.resolution_width,
                        height: mf.resolution_height,
                    });
                    idx += 1;
                }

                if let Some(ref codec) = mf.audio_codec {
                    streams.push(dto::MediaStreamDto {
                        stream_type: "Audio".to_string(),
                        index: idx,
                        codec: Some(codec.clone()),
                        language: None,
                        display_title: Some(codec.clone()),
                        is_default: true,
                        is_forced: false,
                        width: None,
                        height: None,
                    });
                    idx += 1;
                }

                if let Ok(subtitle_tracks) = sf_db::queries::subtitle_tracks::list_by_media_file(&conn, mf.id) {
                    for track in &subtitle_tracks {
                        let display = track.language.as_deref().unwrap_or("Unknown");
                        let mut title = display.to_string();
                        if track.forced {
                            title.push_str(" (Forced)");
                        }
                        streams.push(dto::MediaStreamDto {
                            stream_type: "Subtitle".to_string(),
                            index: idx,
                            codec: Some(track.codec.clone()),
                            language: track.language.clone(),
                            display_title: Some(title),
                            is_default: track.default_track,
                            is_forced: track.forced,
                            width: None,
                            height: None,
                        });
                        idx += 1;
                    }
                }

                dto::MediaSourceDto {
                    id: mf.id.to_string(),
                    name: mf.file_name.clone(),
                    path: mf.file_path.clone(),
                    container: mf.container.clone(),
                    size: Some(mf.file_size),
                    run_time_ticks: ticks,
                    supports_direct_stream: true,
                    supports_direct_play: true,
                    supports_transcoding: false,
                    protocol: "File".to_string(),
                    media_source_type: "Default".to_string(),
                    direct_stream_url: Some(direct_stream_url),
                    media_streams: if streams.is_empty() { None } else { Some(streams) },
                }
            })
            .collect();
        item_dto.media_sources = Some(sources);
    }

    Ok(Json(item_dto))
}

/// GET /Users/{user_id}/Items/{id} -- user-scoped alias for get_item.
pub async fn user_scoped_get_item(
    state: State<AppContext>,
    headers: HeaderMap,
    Path((_user_id, id)): Path<(String, String)>,
) -> Result<Json<BaseItemDto>, AppError> {
    get_item(state, headers, Path(id)).await
}

/// GET /Shows/{id}/Seasons
pub async fn show_seasons(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<ItemsResult>, AppError> {
    let series_id: sf_core::ItemId = id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let children = sf_db::queries::items::list_children(&conn, series_id)?;
    let season_items: Vec<&sf_db::models::Item> = children
        .iter()
        .filter(|c| c.item_kind == "season")
        .collect();
    let season_ids: Vec<sf_core::ItemId> = season_items.iter().map(|i| i.id).collect();
    let mut images_map = sf_db::queries::images::batch_get_images(&conn, &season_ids)?;

    let seasons: Vec<BaseItemDto> = season_items
        .iter()
        .map(|item| {
            let images = images_map.remove(&item.id).unwrap_or_default();
            let mut d = dto::item_to_dto(item, &images, None);
            d.series_id = Some(series_id.to_string());
            d
        })
        .collect();

    let count = seasons.len();
    Ok(Json(ItemsResult {
        items: seasons,
        total_record_count: count,
    }))
}

/// GET /Shows/{id}/Episodes
pub async fn show_episodes(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
    Query(params): Query<ItemsQuery>,
) -> Result<Json<ItemsResult>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;

    // If seasonId is specified, list episodes under that season.
    let parent_id = if let Some(ref season_id) = params.season_id {
        season_id
            .parse::<sf_core::ItemId>()
            .map_err(|_| sf_core::Error::Validation("Invalid season_id".into()))?
    } else {
        // List all episodes for all seasons of this series.
        let series_id: sf_core::ItemId = id
            .parse()
            .map_err(|_| sf_core::Error::Validation("Invalid id".into()))?;

        let seasons = sf_db::queries::items::list_children(&conn, series_id)?;
        let mut all_eps: Vec<(sf_db::models::Item, sf_core::ItemId)> = Vec::new();
        for season in &seasons {
            let eps = sf_db::queries::items::list_children(&conn, season.id)?;
            for ep in eps {
                all_eps.push((ep, season.id));
            }
        }
        let ep_ids: Vec<sf_core::ItemId> = all_eps.iter().map(|(ep, _)| ep.id).collect();
        let mut images_map = sf_db::queries::images::batch_get_images(&conn, &ep_ids)?;

        let episodes: Vec<BaseItemDto> = all_eps
            .iter()
            .map(|(ep, season_id)| {
                let images = images_map.remove(&ep.id).unwrap_or_default();
                let mut d = dto::item_to_dto(ep, &images, None);
                d.series_id = Some(series_id.to_string());
                d.season_id = Some(season_id.to_string());
                d
            })
            .collect();

        let count = episodes.len();
        return Ok(Json(ItemsResult {
            items: episodes,
            total_record_count: count,
        }));
    };

    let children = sf_db::queries::items::list_children(&conn, parent_id)?;
    let ep_items: Vec<&sf_db::models::Item> = children
        .iter()
        .filter(|c| c.item_kind == "episode")
        .collect();
    let ep_ids: Vec<sf_core::ItemId> = ep_items.iter().map(|i| i.id).collect();
    let mut images_map = sf_db::queries::images::batch_get_images(&conn, &ep_ids)?;

    let episodes: Vec<BaseItemDto> = ep_items
        .iter()
        .map(|item| {
            let images = images_map.remove(&item.id).unwrap_or_default();
            dto::item_to_dto(item, &images, None)
        })
        .collect();

    let count = episodes.len();
    Ok(Json(ItemsResult {
        items: episodes,
        total_record_count: count,
    }))
}

/// Well-known anonymous user ID (matches middleware/auth.rs).
fn anonymous_user_id() -> sf_core::UserId {
    "00000000-0000-0000-0000-000000000000"
        .parse()
        .expect("static anonymous UUID is valid")
}

/// Resolve a user ID from Jellyfin request headers.
fn resolve_user_from_headers(ctx: &AppContext, headers: &HeaderMap) -> sf_core::UserId {
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let cookie = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok());
    let x_emby_token = headers
        .get("X-Emby-Token")
        .and_then(|v| v.to_str().ok());

    validate_auth_headers(&ctx.config.auth, &ctx.db, authorization, cookie, x_emby_token)
        .unwrap_or_else(anonymous_user_id)
}

/// GET /Shows/NextUp -- next unwatched episode for the authenticated user.
pub async fn next_up(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(params): Query<ItemsQuery>,
) -> Result<Json<ItemsResult>, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);
    let limit = params.limit.unwrap_or(20).min(100);

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let items = sf_db::queries::playback::next_up(&conn, user_id, limit)?;

    let item_ids: Vec<sf_core::ItemId> = items.iter().map(|i| i.id).collect();
    let mut images_map = sf_db::queries::images::batch_get_images(&conn, &item_ids)?;

    let dtos: Vec<BaseItemDto> = items
        .iter()
        .map(|item| {
            let images = images_map.remove(&item.id).unwrap_or_default();
            let mut d = dto::item_to_dto(item, &images, None);
            // Set series info for episode DTOs.
            if let Some(season_id) = item.parent_id {
                d.season_id = Some(season_id.to_string());
                // Look up series_id from the season's parent.
                if let Ok(Some(season)) = sf_db::queries::items::get_item(&conn, season_id) {
                    if let Some(series_id) = season.parent_id {
                        d.series_id = Some(series_id.to_string());
                        if let Ok(Some(series)) = sf_db::queries::items::get_item(&conn, series_id) {
                            d.series_name = Some(series.name);
                        }
                    }
                }
            }
            d
        })
        .collect();

    let count = dtos.len();
    Ok(Json(ItemsResult {
        items: dtos,
        total_record_count: count,
    }))
}

/// GET /Search/Hints
pub async fn search_hints(
    State(ctx): State<AppContext>,
    Query(params): Query<ItemsQuery>,
) -> Result<Json<SearchHintResult>, AppError> {
    let query = params.search_term.as_deref().unwrap_or("");
    if query.is_empty() {
        return Ok(Json(SearchHintResult {
            search_hints: Vec::new(),
            total_record_count: 0,
        }));
    }

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let items = sf_db::queries::items::search_items(&conn, query, params.limit.unwrap_or(20))?;

    let hints: Vec<SearchHint> = items
        .iter()
        .map(|item| {
            let item_type = match item.item_kind.as_str() {
                "series" => "Series",
                "season" => "Season",
                "episode" => "Episode",
                _ => "Movie",
            };
            SearchHint {
                id: item.id.to_string(),
                name: item.name.clone(),
                item_type: item_type.to_string(),
                production_year: item.year,
            }
        })
        .collect();

    let count = hints.len();
    Ok(Json(SearchHintResult {
        search_hints: hints,
        total_record_count: count,
    }))
}

/// GET /Users/{user_id}/Items/Resume — continue watching row.
pub async fn user_resume(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(_user_id): Path<String>,
    Query(params): Query<ItemsQuery>,
) -> Result<Json<ItemsResult>, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);
    let conn = sf_db::pool::get_conn(&ctx.db)?;

    let limit = params.limit.unwrap_or(12).min(100);
    let offset = params.start_index.unwrap_or(0);

    let kind_strs: Vec<&str> = params
        .include_item_types
        .as_deref()
        .map(|s| s.split(',').filter_map(jellyfin_type_to_kind).collect())
        .unwrap_or_default();
    let kinds = if kind_strs.is_empty() { None } else { Some(kind_strs.as_slice()) };
    let lib_id = params.parent_id.as_deref().and_then(|s| s.parse().ok());

    let items = sf_db::queries::items::list_resumable_items(
        &conn, user_id, lib_id, kinds, offset, limit,
    )?;

    build_response(&conn, user_id, &items)
}

/// Query params for the Latest endpoint.
#[derive(Debug, Deserialize)]
pub struct LatestQuery {
    #[serde(alias = "parentId", alias = "ParentId")]
    pub parent_id: Option<String>,
    #[serde(alias = "limit", alias = "Limit")]
    pub limit: Option<i64>,
    #[serde(alias = "includeItemTypes", alias = "IncludeItemTypes")]
    pub include_item_types: Option<String>,
}

/// GET /Users/{user_id}/Items/Latest — recently added items.
pub async fn user_latest(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(_user_id): Path<String>,
    Query(params): Query<LatestQuery>,
) -> Result<Json<Vec<BaseItemDto>>, AppError> {
    let user_id = resolve_user_from_headers(&ctx, &headers);
    let conn = sf_db::pool::get_conn(&ctx.db)?;

    let limit = params.limit.unwrap_or(16).min(100);
    let lib_id = params.parent_id.as_deref().and_then(|s| s.parse().ok());

    let mut items = sf_db::queries::items::list_latest_items(&conn, lib_id, limit)?;

    // Apply includeItemTypes filter if provided.
    if let Some(ref types) = params.include_item_types {
        items = filter_by_types(items, types);
    }

    let ready = filter_ready_items(&items);
    let item_ids: Vec<sf_core::ItemId> = ready.iter().map(|i| i.id).collect();
    let user_data_map = sf_db::queries::playback::batch_get_user_data(&conn, user_id, &item_ids)?;
    let mut images_map = sf_db::queries::images::batch_get_images(&conn, &item_ids)?;

    let dtos: Vec<BaseItemDto> = ready
        .iter()
        .map(|item| {
            let images = images_map.remove(&item.id).unwrap_or_default();
            let ud = user_data_map.get(&item.id);
            dto::item_to_dto(item, &images, ud)
        })
        .collect();

    // Jellyfin's /Latest returns a bare array, not wrapped in ItemsResult.
    Ok(Json(dtos))
}

/// GET /Users/{user_id}/GroupingOptions — library grouping options.
///
/// Infuse calls this immediately after Views to determine how libraries
/// can be organized. Returns the same libraries as UserViews in a
/// simpler format.
pub async fn grouping_options(
    State(ctx): State<AppContext>,
    Path(_user_id): Path<String>,
) -> Result<Json<Vec<GroupingOption>>, AppError> {
    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let libraries = sf_db::queries::libraries::list_libraries(&conn)?;

    let options: Vec<GroupingOption> = libraries
        .iter()
        .map(|lib| GroupingOption {
            id: lib.id.to_string(),
            name: lib.name.clone(),
        })
        .collect();

    Ok(Json(options))
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GroupingOption {
    pub id: String,
    pub name: String,
}

/// GET /Items/{id}/Images/{image_type}
pub async fn get_image(
    State(ctx): State<AppContext>,
    Path(path): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let (item_id, image_type) = path;
    let id: sf_core::ItemId = item_id
        .parse()
        .map_err(|_| sf_core::Error::Validation("Invalid item_id".into()))?;

    let conn = sf_db::pool::get_conn(&ctx.db)?;
    let images = sf_db::queries::images::list_images_by_item(&conn, id)?;

    let db_type = match image_type.to_lowercase().as_str() {
        "primary" => "primary",
        "backdrop" | "art" | "thumb" => "backdrop",
        _ => "primary",
    };

    let image = images
        .iter()
        .find(|i| i.image_type == db_type)
        .ok_or_else(|| sf_core::Error::not_found("image", format!("{id}/{image_type}")))?;

    let data = tokio::fs::read(&image.path)
        .await
        .map_err(|e| sf_core::Error::Internal(format!("Failed to read image: {e}")))?;

    let content_type = if image.path.ends_with(".png") {
        "image/png"
    } else if image.path.ends_with(".webp") {
        "image/webp"
    } else {
        "image/jpeg"
    };

    Ok((
        StatusCode::OK,
        [
            (axum::http::header::CONTENT_TYPE, content_type),
            (axum::http::header::CACHE_CONTROL, "public, max-age=604800"),
        ],
        data,
    ))
}
