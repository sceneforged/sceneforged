//! OpenAPI documentation and Swagger UI integration.
//!
//! This module provides OpenAPI 3.0 documentation for the Sceneforged API.

use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use super::AppContext;

/// OpenAPI documentation for Sceneforged.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Sceneforged API",
        version = "0.1.0",
        description = "Media automation platform for post-processing with HDR/Dolby Vision support",
        license(name = "MIT", url = "https://opensource.org/licenses/MIT"),
    ),
    servers(
        (url = "/", description = "Default server")
    ),
    paths(
        // API routes (routes_api.rs)
        super::routes_api::health,
        super::routes_api::stats,
        super::routes_api::list_jobs,
        super::routes_api::submit_job,
        super::routes_api::get_job,
        super::routes_api::retry_job,
        super::routes_api::delete_job,
        super::routes_api::get_queue,
        super::routes_api::get_history,
        super::routes_api::get_rules,
        super::routes_api::get_arrs,
        super::routes_api::test_arr,
        super::routes_api::get_tools,
        // Library routes (routes_library.rs)
        super::routes_library::list_libraries,
        super::routes_library::create_library,
        super::routes_library::get_library,
        super::routes_library::delete_library,
        super::routes_library::scan_library,
        super::routes_library::get_library_items,
        super::routes_library::get_recent_items,
        super::routes_library::list_items_handler,
        super::routes_library::get_item,
        super::routes_library::get_children,
        super::routes_library::get_item_files,
        super::routes_library::get_similar_items,
        super::routes_library::search_items,
        // Admin routes (routes_admin.rs)
        super::routes_admin::get_dashboard,
        super::routes_admin::get_streams,
        super::routes_admin::get_library_stats,
        super::routes_admin::get_item_conversion,
        super::routes_admin::convert_item,
        super::routes_admin::batch_convert,
    ),
    components(
        schemas(
            // API types
            super::routes_api::HealthResponse,
            super::routes_api::HealthStats,
            super::routes_api::SubmitJobRequest,
            super::routes_api::SubmitJobResponse,
            super::routes_api::ArrStatus,
            super::routes_api::TestResult,
            super::routes_api::ToolStatusResponse,
            // Library types
            super::routes_library::CreateLibraryRequest,
            super::routes_library::LibraryResponse,
            super::routes_library::ItemResponse,
            super::routes_library::MediaFileResponse,
            super::routes_library::ItemsListResponse,
            super::routes_library::ProviderIdsResponse,
            // Admin types
            super::routes_admin::DashboardResponse,
            super::routes_admin::LibraryStatsResponse,
            super::routes_admin::ProfileCountsResponse,
            super::routes_admin::StreamSessionResponse,
            super::routes_admin::QueueSummaryResponse,
            super::routes_admin::ConversionOptionsResponse,
            super::routes_admin::ConvertItemRequest,
            super::routes_admin::ConvertItemResponse,
            super::routes_admin::BatchConvertRequest,
            super::routes_admin::BatchConvertResponse,
            // State types (re-exported via schemas)
            JobSchema,
            JobStatusSchema,
            JobSourceSchema,
            JobStatsSchema,
            // Config types
            RuleSchema,
            MatchConditionsSchema,
            ActionSchema,
            ResolutionSchema,
            ArrTypeSchema,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "jobs", description = "Job management endpoints"),
        (name = "libraries", description = "Library management endpoints"),
        (name = "items", description = "Media item endpoints"),
        (name = "config", description = "Configuration endpoints"),
        (name = "tools", description = "External tools status"),
        (name = "admin", description = "Admin dashboard and management endpoints"),
    )
)]
pub struct ApiDoc;

// Schema wrappers for types that can't derive ToSchema directly

/// Job information.
#[derive(utoipa::ToSchema)]
#[schema(as = Job)]
pub struct JobSchema {
    /// Unique job identifier
    pub id: String,
    /// Path to the media file
    pub file_path: String,
    /// File name
    pub file_name: String,
    /// Current job status
    pub status: JobStatusSchema,
    /// Name of the matched rule
    pub rule_name: Option<String>,
    /// Progress percentage (0-100)
    pub progress: f32,
    /// Current processing step
    pub current_step: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// When the job was created
    pub created_at: String,
    /// When processing started
    pub started_at: Option<String>,
    /// When processing completed
    pub completed_at: Option<String>,
    /// Source of the job
    pub source: JobSourceSchema,
}

/// Job status.
#[derive(utoipa::ToSchema)]
#[schema(as = JobStatus)]
pub enum JobStatusSchema {
    #[schema(rename = "queued")]
    Queued,
    #[schema(rename = "running")]
    Running,
    #[schema(rename = "completed")]
    Completed,
    #[schema(rename = "failed")]
    Failed,
    #[schema(rename = "cancelled")]
    Cancelled,
}

/// Source of a job.
#[derive(utoipa::ToSchema)]
#[schema(as = JobSource)]
pub enum JobSourceSchema {
    /// Job submitted via webhook
    Webhook {
        arr_name: String,
        item_id: Option<i64>,
    },
    /// Job detected by file watcher
    Watcher { watch_path: String },
    /// Job submitted manually via CLI
    Manual,
    /// Job submitted via API
    Api,
}

/// Processing statistics.
#[derive(utoipa::ToSchema)]
#[schema(as = JobStats)]
pub struct JobStatsSchema {
    /// Total number of processed jobs
    pub total_processed: u64,
    /// Number of successful jobs
    pub successful: u64,
    /// Number of failed jobs
    pub failed: u64,
    /// Total bytes processed
    pub total_bytes_processed: u64,
}

/// Processing rule.
#[derive(utoipa::ToSchema)]
#[schema(as = Rule)]
pub struct RuleSchema {
    /// Rule name
    pub name: String,
    /// Whether the rule is enabled
    pub enabled: bool,
    /// Priority (higher = matched first)
    pub priority: i32,
    /// Match conditions
    #[schema(rename = "match")]
    pub match_conditions: MatchConditionsSchema,
    /// Actions to perform when matched
    pub actions: Vec<ActionSchema>,
}

/// Conditions for matching media files.
#[derive(utoipa::ToSchema)]
#[schema(as = MatchConditions)]
pub struct MatchConditionsSchema {
    /// Video codecs to match
    pub codecs: Vec<String>,
    /// Container formats to match
    pub containers: Vec<String>,
    /// HDR formats to match (sdr, hdr10, hdr10+, dolbyvision, hlg)
    pub hdr_formats: Vec<String>,
    /// Dolby Vision profiles to match
    pub dolby_vision_profiles: Vec<u8>,
    /// Minimum resolution
    pub min_resolution: Option<ResolutionSchema>,
    /// Maximum resolution
    pub max_resolution: Option<ResolutionSchema>,
    /// Audio codecs to match
    pub audio_codecs: Vec<String>,
}

/// Video resolution.
#[derive(utoipa::ToSchema)]
#[schema(as = Resolution)]
pub struct ResolutionSchema {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

/// Processing action.
#[derive(utoipa::ToSchema)]
#[schema(as = Action)]
pub enum ActionSchema {
    /// Convert Dolby Vision profile
    DvConvert { target_profile: u8 },
    /// Remux to different container
    Remux {
        container: String,
        keep_original: bool,
    },
    /// Add compatibility audio track
    AddCompatAudio {
        source_codec: String,
        target_codec: String,
    },
    /// Strip tracks by type or language
    StripTracks {
        track_types: Vec<String>,
        languages: Vec<String>,
    },
    /// Execute external command
    Exec { command: String, args: Vec<String> },
}

/// Arr integration type.
#[derive(utoipa::ToSchema)]
#[schema(as = ArrType)]
pub enum ArrTypeSchema {
    #[schema(rename = "radarr")]
    Radarr,
    #[schema(rename = "sonarr")]
    Sonarr,
}

/// Create OpenAPI documentation routes.
/// - `/docs` - Swagger UI
/// - `/openapi.json` - Raw OpenAPI JSON spec (served by SwaggerUi)
pub fn openapi_routes() -> Router<AppContext> {
    Router::new().merge(SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
}
