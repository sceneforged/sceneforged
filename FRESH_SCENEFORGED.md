# Sceneforged: Fresh Build Orchestration Plan

This document is a self-contained blueprint for an orchestrator agent to build Sceneforged from scratch in an empty folder. The orchestrator spawns task subagents, checks their work with a fast model, and drives tasks to completion.

> **Reference codebase:** `/Users/dallas/git/sceneforged` — consult for domain logic and behavior reference, but do NOT copy code wholesale. Reimplement with the architecture described here.

---

## Library Versions (validated Feb 2026)

### Rust (Cargo.toml workspace dependencies)

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.49 | Async runtime |
| axum | 0.8 | Web framework |
| tower-http | 0.6 | HTTP middleware (CORS, tracing, compression) |
| serde | 1.0 | Serialization |
| serde_json | 1.0 | JSON |
| thiserror | 2.0 | Error derive |
| tracing | 0.1 | Structured logging |
| tracing-subscriber | 0.3 | Log formatting (features: json, env-filter) |
| clap | 4.5 | CLI parsing (features: derive) |
| uuid | 1.18 | IDs (features: v4, serde) |
| rusqlite | 0.38 | SQLite (features: bundled) |
| r2d2 | 0.8 | Connection pooling |
| r2d2_sqlite | 0.25 | SQLite r2d2 adapter |
| chrono | 0.4 | Date/time (features: serde) |
| notify | 8.2 | File watching |
| governor | 0.8 | Rate limiting |
| metrics | 0.24 | Metrics facade |
| metrics-exporter-prometheus | 0.18 | Prometheus exporter |
| utoipa | 5.4 | OpenAPI generation |
| utoipa-swagger-ui | 9.0 | Swagger UI |
| semver | 1.0 | Version parsing |
| async-trait | 0.1 | Async traits |
| tokio-util | 0.7 | CancellationToken, etc. |
| reqwest | 0.12 | HTTP client |
| image | 0.25 | Image processing |
| dolby_vision | 3.3 | DV RPU parsing |
| matroska | 0.29 | MKV parsing |
| mp4parse | 0.17 | MP4 parsing |
| bitstream-io | 2.6 | Bitstream reading |
| winnow | 0.7 | Parser combinators |
| logos | 0.14 | Lexer generator |
| tempfile | 3.15 | Temp directories |
| which | 7.0 | PATH lookup |
| parking_lot | 0.12 | Fast RwLock/Mutex |
| proptest | 1.6 | Property-based testing |
| wiremock | 0.6 | HTTP mock server |
| assert_cmd | 2.0 | CLI testing |

### Frontend (ui/package.json)

| Package | Version | Purpose |
|---------|---------|---------|
| svelte | ^5.47 | UI framework |
| @sveltejs/kit | ^2.50 | App framework |
| @sveltejs/adapter-static | ^3.0 | Static SPA build |
| vite | ^7.3 | Build tool |
| typescript | ^5.9 | Type system |
| tailwindcss | ^4.1 | Styling |
| @tailwindcss/vite | ^4.1 | Vite plugin |
| bits-ui | ^2.15 | Headless UI primitives |
| lucide-svelte | ^0.562 | Icons |
| svelte-sonner | ^1.0 | Toast notifications |
| hls.js | ^1.6 | HLS video playback |
| openapi-fetch | ^0.15 | Typed API client |
| openapi-typescript | ^7.6 | Type generation from OpenAPI |
| clsx | ^2.1 | Conditional classes |
| tailwind-merge | ^3.0 | Class deduplication |
| vitest | ^3.0 | Unit testing |
| @testing-library/svelte | ^5.2 | Component testing |
| playwright | ^1.50 | E2E testing |

### Docker

| Image | Tag | Purpose |
|-------|-----|---------|
| node | 22-alpine | UI build stage |
| rust | 1-trixie | Rust build (FFmpeg 7+ headers) |
| debian | trixie-slim | Runtime |

---

## Task Definitions

Each task has: ID, title, description, blocked-by list, and a prompt template for the subagent.

---

### TASK-01: Initialize workspace and Cargo.toml

**Blocked by:** nothing
**Description:** Create the workspace directory structure with all 9 crates, the binary crate, and the UI scaffold. Set up `Cargo.toml` at root with workspace dependencies using the versions table above. Each crate gets its own `Cargo.toml` with appropriate dependencies. Create `rust-toolchain.toml` targeting stable. Create `.gitignore`.

**Prompt template:**
```
You are implementing TASK-01 for a fresh Rust + SvelteKit project called "sceneforged".

Create the full workspace directory structure in the current (empty) directory:

sceneforged/
  Cargo.toml                    # Workspace root with [workspace.dependencies]
  rust-toolchain.toml           # channel = "stable"
  .gitignore                    # Rust + Node + IDE ignores
  crates/
    sf-core/Cargo.toml + src/lib.rs
    sf-probe/Cargo.toml + src/lib.rs
    sf-av/Cargo.toml + src/lib.rs
    sf-rules/Cargo.toml + src/lib.rs
    sf-pipeline/Cargo.toml + src/lib.rs
    sf-db/Cargo.toml + src/lib.rs
    sf-media/Cargo.toml + src/lib.rs
    sf-parser/Cargo.toml + src/lib.rs
    sf-server/Cargo.toml + src/lib.rs
  src/main.rs                   # Binary crate (just a stub for now)

Use workspace dependencies (declare versions once at root, reference with `.workspace = true` in crates).

Dependency graph:
- sf-core: serde, serde_json, thiserror, uuid, chrono, tracing
- sf-probe: sf-core, matroska, mp4parse, dolby_vision, bitstream-io
- sf-av: sf-core, sf-probe, which, tokio (process), tempfile
- sf-rules: sf-core, sf-probe, serde
- sf-pipeline: sf-core, sf-probe, sf-av, sf-rules, tokio, tokio-util, tempfile
- sf-db: sf-core, rusqlite (bundled), r2d2, r2d2_sqlite, chrono
- sf-media: sf-core
- sf-parser: sf-core, winnow, logos
- sf-server: sf-core, sf-db, sf-pipeline, sf-media, sf-rules, sf-probe, axum, tower-http, tokio, utoipa, serde_json, metrics, metrics-exporter-prometheus, governor, notify, reqwest, image
- binary (src/): sf-server, sf-core, clap, tokio, tracing, tracing-subscriber

Library versions to use:
{INSERT_VERSIONS_TABLE_FROM_ABOVE}

After creating all files, run `cargo check` to verify the workspace compiles.
Do NOT create the UI directory yet — that is a separate task.
```

---

### TASK-02: Implement sf-core domain types

**Blocked by:** TASK-01
**Description:** Implement the foundational types all crates depend on: typed ID macro, Error enum with HTTP status mapping, media domain enums (Container, VideoCodec, AudioCodec, HdrFormat, Profile, ItemKind, FileRole, StreamType, ImageType), config schema structs, and event types.

**Prompt template:**
```
You are implementing TASK-02: the sf-core crate.

Reference: /Users/dallas/git/sceneforged/crates/sceneforged-common/src/ for ID patterns and /Users/dallas/git/sceneforged/src/config/types.rs for config schema and /Users/dallas/git/sceneforged/src/state/mod.rs for event types.

DO NOT copy code wholesale. Reimplement cleanly with these improvements:

1. **Typed IDs** (src/ids.rs): Create a `typed_id!` macro that generates a newtype over Uuid with Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, FromStr. Generate: JobId, ItemId, LibraryId, MediaFileId, UserId, SessionId, ImageId, ConversionJobId, RuleId.

2. **Error types** (src/error.rs): Single Error enum with variants: NotFound{entity, id}, Unauthorized(String), Forbidden(String), Validation(String), Conflict(String), Database{source: Box<dyn Error + Send + Sync>}, Io{source: io::Error}, Tool{tool, message}, Probe(String), Pipeline{step, message}, Internal(String). Add `http_status(&self) -> u16` method. Add `pub type Result<T> = std::result::Result<T, Error>;`.

3. **Media enums** (src/media.rs): Container(Mkv, Mp4), VideoCodec(H264, H265, Av1, Vp9), AudioCodec(Aac, Ac3, Eac3, TrueHd, Dts, DtsHd, Flac, Opus), HdrFormat(Sdr, Hdr10, Hdr10Plus, DolbyVision, Hlg), Profile(A, B, C), ItemKind(Movie, Series, Season, Episode), FileRole(Source, Universal, Extra), StreamType(Video, Audio, Subtitle), ImageType(Primary, Backdrop, Banner, Thumb, Logo). All with Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Display.

4. **Config types** (src/config.rs): ServerConfig, AuthConfig, WatchConfig, ArrConfig, JellyfinConfig, ToolsConfig, ConversionConfig, MetadataConfig, ImageConfig, WebhookSecurityConfig. Top-level Config struct. All with serde(default) and Deserialize. Add `Config::from_file(path)`, `Config::validate() -> Vec<ValidationError>`, `Config::load_or_default(path: Option<&Path>)`.

5. **Event types** (src/events.rs): EventCategory(Admin, User), EventPayload enum with variants for job lifecycle (JobQueued, JobStarted, JobProgress, JobCompleted, JobFailed), library lifecycle (LibraryScanStarted/Progress/Complete, LibraryCreated/Deleted), item lifecycle (ItemAdded/Updated/Removed), conversion (ConversionProgress/Completed/Failed). Event struct wrapping id, timestamp, category, payload. EventBus struct with broadcast::Sender, recent events ring buffer, subscribe(), broadcast(), recent_events(n).

6. **Module structure**: lib.rs re-exports all modules.

After implementing, run `cargo check -p sf-core` and `cargo test -p sf-core`.
```

---

### TASK-03: Implement sf-probe (pure Rust media probing)

**Blocked by:** TASK-02
**Description:** Implement pure Rust probing of MKV and MP4 files: container detection, video/audio/subtitle track extraction, HDR/Dolby Vision detection. Define the `Prober` trait.

**Prompt template:**
```
You are implementing TASK-03: the sf-probe crate.

Reference: /Users/dallas/git/sceneforged/crates/sceneforged-probe/src/ for the pure Rust probing logic.

Implement:

1. **MediaInfo struct** (src/types.rs): The canonical probe result type. Fields: file_path, file_size, container (sf_core::Container), duration (Option<Duration>), video_tracks (Vec<VideoTrack>), audio_tracks (Vec<AudioTrack>), subtitle_tracks (Vec<SubtitleTrack>). VideoTrack has: codec, width, height, frame_rate, bit_depth, hdr_format, dolby_vision (Option<DvInfo>), default, language. AudioTrack has: codec, channels, sample_rate, language, atmos, default. SubtitleTrack has: codec, language, forced, default. DvInfo has: profile, rpu_present, el_present, bl_present. Add `primary_video()` and `primary_audio()` helper methods.

2. **Prober trait** (src/trait.rs): `trait Prober: Send + Sync { fn name() -> &'static str; fn probe(path: &Path) -> sf_core::Result<MediaInfo>; fn supports(path: &Path) -> bool; }`

3. **RustProber** (src/rust_prober.rs): Implements Prober. Detects container from file magic bytes. For MKV: use matroska crate to parse tracks, extract codec info, detect HDR from codec private data. For MP4: use mp4parse crate. For HEVC tracks: parse SPS NAL units for VUI (HDR10/HLG), parse SEI for HDR10+, parse RPU for Dolby Vision using dolby_vision crate.

4. **CompositeProber** (src/composite.rs): Takes Vec<Box<dyn Prober>>, tries each in order, returns first success. Falls back through backends on error.

5. Re-export key types from lib.rs.

After implementing, run `cargo check -p sf-probe` and `cargo test -p sf-probe`.
```

---

### TASK-04: Implement sf-av (external tool orchestration)

**Blocked by:** TASK-02, TASK-03
**Description:** Implement the ToolRegistry, ToolCommand execution with timeouts, and external probe backends (FfprobeProber, MediaInfoProber). Implement action functions (remux, dv_convert, add_compat_audio, strip_tracks) and Workspace abstraction.

**Prompt template:**
```
You are implementing TASK-04: the sf-av crate.

Reference: /Users/dallas/git/sceneforged/crates/sceneforged-av/src/ for tool management and action implementations.

Implement:

1. **ToolRegistry** (src/tools.rs): Struct holding HashMap<String, ToolConfig>. ToolConfig has: name, path (PathBuf), min_version (Option<semver::VersionReq>), timeout (Duration, default 5min). Methods: `discover(config: &ToolsConfig) -> Self` (checks PATH via `which` crate, validates versions), `require(name: &str) -> Result<&ToolConfig>`, `check_all() -> Vec<ToolInfo>`. ToolInfo has: name, available, version, path.

2. **ToolCommand** (src/command.rs): Builder for executing external tools. Fields: tool path, args, timeout, optional stdin. `async execute() -> Result<ToolOutput>` uses tokio::process::Command with tokio::time::timeout. ToolOutput has: status (ExitStatus), stdout (String), stderr (String). On timeout: return Error::Tool with message including timeout duration.

3. **Workspace** (src/workspace.rs): Manages temp directory for pipeline operations. `new(input: &Path) -> Result<Self>`, `input() -> &Path`, `output() -> &Path`, `temp_dir() -> &Path`, `finalize(backup_ext: Option<&str>) -> Result<PathBuf>` (atomically replaces input with output, optionally backs up original).

4. **Probe backends** (src/probe/): FfprobeProber (implements sf_probe::Prober, shells to ffprobe -print_format json, parses output into MediaInfo), MediaInfoProber (implements sf_probe::Prober, shells to mediainfo --Output=JSON).

5. **Action functions** (src/actions/): Each takes &Workspace and config params, returns Result<()>.
   - `remux(ws, target_container)` — uses ffmpeg or mkvmerge
   - `convert_dv_profile(ws, target_profile)` — uses dolby_vision crate (pure Rust) or dovi_tool CLI
   - `add_compat_audio(ws, source_codec, target_codec)` — uses ffmpeg
   - `strip_tracks(ws, config)` — uses mkvmerge
   - `exec_command(ws, command, args)` — runs arbitrary command

After implementing, run `cargo check -p sf-av` and `cargo test -p sf-av`.
```

---

### TASK-05: Implement sf-rules (rule engine)

**Blocked by:** TASK-02, TASK-03
**Description:** Implement the expression tree rule engine with AND/OR/NOT support, TOML deserialization with backwards-compatible simple form, evaluation against MediaInfo, and rule sorting by priority.

**Prompt template:**
```
You are implementing TASK-05: the sf-rules crate.

Reference: /Users/dallas/git/sceneforged/src/rules/ for current rule matching logic and /Users/dallas/git/sceneforged/src/config/types.rs for rule/action config types.

Implement:

1. **Condition enum** (src/condition.rs): Leaf conditions: Codec(Vec<VideoCodec>), Container(Vec<Container>), HdrFormat(Vec<HdrFormat>), DolbyVisionProfile(Vec<u8>), MinResolution{width, height}, MaxResolution{width, height}, AudioCodec(Vec<AudioCodec>), HasAtmos(bool), MinBitDepth(u8), FileExtension(Vec<String>). Each has an `evaluate(&self, info: &MediaInfo) -> bool` method.

2. **Expr enum** (src/expr.rs): `Condition(Condition)`, `And(Vec<Expr>)`, `Or(Vec<Expr>)`, `Not(Box<Expr>)`. `evaluate(expr: &Expr, info: &MediaInfo) -> bool` function.

3. **ActionConfig enum** (src/action_config.rs): DvConvert{target_profile}, Remux{container, keep_original}, AddCompatAudio{source_codec, target_codec}, StripTracks{track_types, languages}, Exec{command, args}. With Serialize/Deserialize.

4. **Rule struct** (src/rule.rs): id (RuleId), name (String), enabled (bool), priority (i32), expr (Expr), actions (Vec<ActionConfig>).

5. **TOML deserialization** (src/serde_compat.rs): Support both simple form (flat match conditions → implicit AND) and advanced form (any_of, not keys). Custom Deserialize impl for Expr that handles both forms.

6. **RuleEngine** (src/engine.rs): Holds Vec<Rule> sorted by priority desc. `find_matching_rule(info: &MediaInfo) -> Option<&Rule>` returns first match. `evaluate_all(info: &MediaInfo) -> Vec<&Rule>` returns all matches.

After implementing, run `cargo check -p sf-rules` and `cargo test -p sf-rules`. Include property-based tests with proptest for expression evaluation consistency.
```

---

### TASK-06: Implement sf-pipeline (action execution)

**Blocked by:** TASK-04, TASK-05
**Description:** Implement the Action trait, pipeline executor with parallel stage support, rollback on failure, dry-run validation, progress reporting, and cancellation.

**Prompt template:**
```
You are implementing TASK-06: the sf-pipeline crate.

Reference: /Users/dallas/git/sceneforged/src/pipeline/ for current pipeline executor logic.

Implement:

1. **Action trait** (src/action.rs): `#[async_trait] trait Action: Send + Sync` with methods: `name() -> &'static str`, `validate(ctx: &ActionContext) -> Result<()>`, `execute(ctx: &ActionContext) -> Result<ActionResult>`, `rollback(ctx: &ActionContext) -> Result<()>` (default: Ok(())), `parallelizable() -> bool` (default: false), `weight() -> f32` (default: 1.0).

2. **ActionContext** (src/context.rs): workspace (Arc<sf_av::Workspace>), media_info (Arc<sf_probe::MediaInfo>), dry_run (bool), cancellation (tokio_util::sync::CancellationToken), progress (ProgressSender). ProgressSender wraps a callback `Box<dyn Fn(f32, &str) + Send + Sync>`.

3. **Built-in actions** (src/actions/): DvConvertAction, RemuxAction, AddCompatAudioAction, StripTracksAction, ExecAction. Each constructed from ActionConfig, delegates to sf_av functions.

4. **PipelineExecutor** (src/executor.rs): Takes Vec<Box<dyn Action>>. `async execute(ctx: ActionContext) -> Result<PathBuf>` method: groups actions into sequential stages (parallelizable actions within a stage run concurrently via try_join_all), tracks completed actions, on failure calls rollback in reverse order. Reports weighted progress. Checks cancellation token between stages.

5. **Action factory** (src/factory.rs): `fn create_actions(configs: &[ActionConfig], tools: &ToolRegistry) -> Result<Vec<Box<dyn Action>>>` — validates tool availability, constructs action objects.

After implementing, run `cargo check -p sf-pipeline` and `cargo test -p sf-pipeline`.
```

---

### TASK-07: Implement sf-db (database layer)

**Blocked by:** TASK-02
**Description:** Implement SQLite database with connection pooling, migrations, transaction wrapper, repository pattern for all entities (users, libraries, items, media_files, jobs, playback, images, hls_cache, auth_tokens, conversion_jobs).

**Prompt template:**
```
You are implementing TASK-07: the sf-db crate.

Reference: /Users/dallas/git/sceneforged/crates/sceneforged-db/src/ for current schema and query patterns.

Implement:

1. **Pool** (src/pool.rs): `init_pool(path: &str) -> Result<DbPool>` and `init_memory_pool() -> Result<DbPool>` using r2d2 + r2d2_sqlite. Max 4 connections. Enable foreign keys and WAL mode on each connection via CustomizeConnection.

2. **Migrations** (src/migrations/): Embedded SQL files. Migration runner that tracks applied migrations in `schema_migrations` table with version number and checksum. Runs on pool init. At minimum create these tables:
   - users (id, username, password_hash, role, created_at)
   - auth_tokens (id, user_id, token, expires_at)
   - libraries (id, name, media_type, paths JSON, config JSON, created_at)
   - items (id, library_id, item_kind, name, sort_name, year, overview, runtime_minutes, community_rating, provider_ids JSON, parent_id nullable, season_number, episode_number, created_at, updated_at)
   - media_files (id, item_id, file_path, file_name, file_size, container, video_codec, audio_codec, resolution_width, resolution_height, hdr_format, has_dolby_vision, dv_profile, role, profile, duration_secs, created_at)
   - images (id, item_id, image_type, path, provider, width, height)
   - jobs (id, file_path, file_name, status, rule_name, progress, current_step, error, source, retry_count, max_retries, priority, locked_by, locked_at, created_at, started_at, completed_at, scheduled_for)
   - conversion_jobs (id, item_id, media_file_id, status, progress_pct, encode_fps, eta_secs, error, created_at, started_at, completed_at)
   - hls_cache (media_file_id, playlist, segments JSON, created_at)
   - playback (user_id, item_id, position_secs, completed, play_count, last_played_at)

3. **Transaction wrapper** (src/transaction.rs): `Tx<'a>` wrapping rusqlite::Transaction. `begin(conn)`, `commit()`, auto-rollback on drop.

4. **Repository modules** (src/queries/): One module per entity (users.rs, libraries.rs, items.rs, media_files.rs, jobs.rs, conversion_jobs.rs, images.rs, playback.rs, hls_cache.rs, auth.rs). Each has functions taking &Connection or &Tx for transactional operations. Jobs module has atomic `dequeue_next() -> Option<Job>` using UPDATE...RETURNING. Items module uses LEFT JOIN to eager-load images and profile flags.

5. **Models** (src/models.rs): Rust structs matching each table. All with appropriate From<&Row> implementations.

After implementing, run `cargo check -p sf-db` and `cargo test -p sf-db` (tests should use init_memory_pool).
```

---

### TASK-08: Implement sf-parser (release name parsing)

**Blocked by:** TASK-02
**Description:** Implement the release name parser using winnow and logos. This is well-implemented in the reference codebase — study the approach but reimplement cleanly.

**Prompt template:**
```
You are implementing TASK-08: the sf-parser crate.

Reference: /Users/dallas/git/sceneforged/crates/sceneforged-parser/src/ for the current parser implementation. This crate is already well-architected in the reference — study its approach carefully.

Implement a release name parser that extracts metadata from filenames like "The.Matrix.1999.1080p.BluRay.x264-GROUP":

1. **Output struct**: ParsedRelease with fields: title, year, resolution, source (BluRay/WEB/HDTV/etc), video_codec, audio_codec, languages, edition, group, revision.

2. **Tokenizer** (src/tokenizer.rs): Use logos to define tokens for known keywords (codec names, sources, resolutions, etc.).

3. **Parser** (src/parser.rs): Use winnow combinators to parse token sequences into ParsedRelease.

4. **Keyword tables**: Use phf (perfect hash function) for fast keyword lookups of codec variants, source variants, etc.

5. **pub fn parse(input: &str) -> ParsedRelease** as the main entry point.

Include comprehensive unit tests with various release name formats.

After implementing, run `cargo check -p sf-parser` and `cargo test -p sf-parser`.
```

---

### TASK-09: Implement sf-media (HLS and fMP4)

**Blocked by:** TASK-02
**Description:** Implement fMP4 serialization and HLS playlist generation for streaming support.

**Prompt template:**
```
You are implementing TASK-09: the sf-media crate.

Reference: /Users/dallas/git/sceneforged/crates/sceneforged-media/src/ for the current fMP4 and HLS implementation.

Implement:

1. **fMP4 writer** (src/fmp4/): Serialize ISO BMFF (fragmented MP4) segments. Support init segments and media segments for both video (H.264/H.265) and audio (AAC). This involves writing MP4 boxes: ftyp, moov (with mvhd, trak, mvex), moof (with mfhd, traf), mdat.

2. **HLS playlist generator** (src/hls/): Generate M3U8 master playlists and media playlists. Support: variant streams (multiple qualities), segment duration targeting, EXT-X-MAP for init segments, EXT-X-TARGETDURATION, EXT-X-MEDIA-SEQUENCE. `fn generate_master_playlist(variants: &[Variant]) -> String` and `fn generate_media_playlist(segments: &[Segment]) -> String`.

3. **Segment map** (src/segment_map.rs): Pre-compute segment boundaries from media file keyframes. Store as serializable struct for caching in database.

After implementing, run `cargo check -p sf-media` and `cargo test -p sf-media`.
```

---

### TASK-10: Implement sf-server (web server)

**Blocked by:** TASK-02, TASK-03, TASK-05, TASK-06, TASK-07, TASK-09
**Description:** Implement the Axum web server with all routes, middleware, SSE, auth, rate limiting, and OpenAPI documentation.

**Prompt template:**
```
You are implementing TASK-10: the sf-server crate.

Reference: /Users/dallas/git/sceneforged/src/server/ for route handlers and /Users/dallas/git/sceneforged/src/state/mod.rs for event broadcasting.

Implement:

1. **AppContext** (src/context.rs): Service-oriented context struct (Clone via Arc) with: db (DbPool), config (Arc<Config>), config_store (Arc<ConfigStore>), event_bus (sf_core::EventBus), prober (Arc<dyn Prober>), tools (Arc<ToolRegistry>). ConfigStore wraps mutable runtime config (rules, arrs, jellyfins, conversion) with hot-reload via notify file watcher and persist-to-TOML.

2. **Router** (src/router.rs): Build the Axum router:
   - GET /health
   - POST /api/auth/login, POST /api/auth/logout, GET /api/auth/status
   - Protected routes (behind auth middleware):
     - GET/POST /api/libraries, GET/DELETE /api/libraries/:id, POST /api/libraries/:id/scan
     - GET /api/items, GET /api/items/:id
     - GET /api/jobs, POST /api/jobs/submit, GET /api/jobs/:id, POST /api/jobs/:id/retry, DELETE /api/jobs/:id
     - GET /api/events (SSE)
     - GET /api/config/rules, PUT /api/config/rules, GET /api/config/arrs
     - GET /api/stream/hls/:item_id/master.m3u8, GET /api/stream/hls/:item_id/:segment
     - GET /api/images/:item_id/:type/:size
     - GET /api/admin/dashboard, GET /api/admin/tools
   - POST /webhook/:arr_name (with optional signature verification)
   - GET /metrics (Prometheus)
   - Static file serving for UI build

3. **Middleware** (src/middleware/):
   - request_id: Generate/extract x-request-id, add to tracing span, return in response
   - auth: Session cookie or API key validation, skip for /health and /api/auth/*
   - rate_limit: governor-based, 300/min for API, 30/min for webhooks

4. **SSE handler** (src/routes/events.rs): Subscribe to EventBus, filter by category query param, replay recent events for late joiners, keep-alive heartbeat every 15s.

5. **Error responses** (src/error.rs): Implement IntoResponse for sf_core::Error. Return structured JSON: { error, code, request_id }.

6. **Job processor** (src/processor.rs): Background tokio task. Polls database for queued jobs (atomic dequeue). Probes file, matches rules, executes pipeline, updates job status. Retry with exponential backoff. Emits events for each lifecycle change.

7. **File watcher** (src/watcher.rs): Background task using notify crate. Watches configured directories, queues jobs for new files matching extensions, with settle time.

8. **Startup function** (src/lib.rs): `pub async fn start(config: Config, config_path: Option<PathBuf>) -> Result<()>` — initializes DB, creates AppContext, starts processor + watcher + server, handles graceful shutdown.

After implementing, run `cargo check -p sf-server` and `cargo test -p sf-server`.
```

---

### TASK-11: Implement binary crate (CLI)

**Blocked by:** TASK-10
**Description:** Implement the thin binary crate with CLI parsing via clap. Commands: start, run, probe, check-tools, validate, version, hash-password, generate-api-key, generate-secret.

**Prompt template:**
```
You are implementing TASK-11: the binary crate (src/main.rs and src/cli.rs).

Reference: /Users/dallas/git/sceneforged/src/main.rs and /Users/dallas/git/sceneforged/src/cli.rs for CLI structure.

Implement:

1. **CLI definition** (src/cli.rs): Using clap derive. Cli struct with global options: --config (path), --verbose. Commands enum: Start{host, port}, Run{input, dry_run, force}, Probe{file, json}, CheckTools, Validate{config}, Version, HashPassword{password}, GenerateApiKey, GenerateSecret.

2. **main.rs**: Parse CLI, init tracing (respect RUST_LOG env var, verbose flag increases to trace), dispatch to command handlers. Start command calls sf_server::start(). Run command: load config, probe file, match rules, execute pipeline. Probe command: probe file, print results (text or JSON). CheckTools: call tool registry discover, print status. Validate: load and validate config, print results.

Keep this file under 200 lines. All logic lives in the library crates.

After implementing, run `cargo build` and test CLI commands: `cargo run -- version`, `cargo run -- validate`, `cargo run -- check-tools`.
```

---

### TASK-12: Initialize frontend (SvelteKit + Tailwind)

**Blocked by:** TASK-01
**Description:** Create the SvelteKit 5 frontend with Tailwind CSS 4, bits-ui, and the complete project scaffold including theming, layout, and type generation setup.

**Prompt template:**
```
You are implementing TASK-12: the SvelteKit frontend scaffold.

Create the ui/ directory with a complete SvelteKit 5 project:

1. **Project setup**: Use `npm create svelte@latest` patterns. Configure:
   - svelte.config.js with adapter-static (SPA mode, fallback: index.html)
   - vite.config.ts with @tailwindcss/vite plugin and dev proxy (/api -> localhost:8080)
   - tsconfig.json with strict mode
   - package.json with all dependencies from the versions table

2. **Tailwind CSS 4** (src/app.css): Import tailwindcss. Define CSS variables for theming in oklch color space (light/dark mode). Variables: --background, --foreground, --card, --primary, --secondary, --muted, --accent, --destructive, --border, --input, --ring, --radius.

3. **Dark mode** (src/app.html): Add script before body to check localStorage/system preference and apply .dark class (prevents flash).

4. **Utility** (src/lib/utils.ts): `cn()` function combining clsx + tailwind-merge.

5. **Layout** (src/routes/+layout.svelte, +layout.ts): Set ssr=false, prerender=false. Create responsive layout with sidebar (desktop) and mobile nav. Use Svelte 5 runes for sidebar state.

6. **UI primitives** (src/lib/components/ui/): Set up bits-ui based components: button (with variants), card, badge, dialog, input, select, progress, alert, tabs, table, separator, scroll-area, skeleton, dropdown-menu. Each in its own directory with index.ts barrel export.

7. **Type generation script**: In package.json add "generate:types" script using openapi-typescript.

8. **Routes stub**: Create route files (just placeholder pages) for: /, /login, /browse/[libraryId], /browse/[libraryId]/[itemId], /play/[itemId], /rules, /settings, /admin, /admin/jobs, /admin/libraries, /admin/libraries/[libraryId], /admin/item/[itemId].

After creating, run `cd ui && pnpm install && pnpm check && pnpm build`.
```

---

### TASK-13: Implement frontend stores and API client

**Blocked by:** TASK-12
**Description:** Implement all stores using Svelte 5 runes exclusively, the API client with caching/retry/deduplication, and the SSE events service.

**Prompt template:**
```
You are implementing TASK-13: frontend state management and API layer.

Reference: /Users/dallas/git/sceneforged/ui/src/lib/stores/ and /Users/dallas/git/sceneforged/ui/src/lib/api.ts and /Users/dallas/git/sceneforged/ui/src/lib/services/events.svelte.ts for current patterns.

ALL stores must use Svelte 5 runes ($state, $derived, $effect). Do NOT use writable() from svelte/store.

1. **API Client** (src/lib/api/client.ts): Class with: cache (Map with TTL), inflight request dedup (Map of Promises), methods: get<T>(endpoint, opts?), post<T>(endpoint, body), put<T>(endpoint, body), delete(endpoint). Automatic retry (3 attempts, exponential backoff). Adds auth headers. Returns structured ApiError on failure. `invalidate(pattern)` clears cache entries.

2. **API functions** (src/lib/api/index.ts): Typed functions using the client: getLibraries, getLibrary, createLibrary, deleteLibrary, scanLibrary, getItems, getItem, getJobs, submitJob, retryJob, deleteJob, getConfigRules, updateConfigRules, getDashboard, getTools, login, logout, getAuthStatus.

3. **Auth store** (src/lib/stores/auth.svelte.ts): Runes-based. State: authenticated, username, authEnabled, initialized. Methods: checkStatus, login, logout. Export as object with getters.

4. **Theme store** (src/lib/stores/theme.svelte.ts): Runes-based. State: theme ('light'|'dark'|'system'). Persist to localStorage. Apply .dark class to document. Methods: set, toggle, cycle.

5. **Jobs store** (src/lib/stores/jobs.svelte.ts): Runes-based. State: activeJobs, jobHistory. Methods: refresh, handleEvent (processes SSE events to update state). Derived: runningJobs, queuedJobs.

6. **Library store** (src/lib/stores/library.svelte.ts): Runes-based. State: libraries, selectedLibrary, items, loading, searchQuery, pagination. Methods: loadLibraries, selectLibrary, loadItems, search, setPage.

7. **Events service** (src/lib/services/events.svelte.ts): Runes-based. Manages EventSource connection with exponential backoff + jitter reconnection. Category-based subscriber filtering (admin/user/all). Methods: connect, disconnect, subscribe(filter, callback) returns unsubscribe function.

8. **Types** (src/lib/types.ts): Define TypeScript interfaces for all domain types: Job, Rule, Item, Library, MediaFile, AppEvent (discriminated union), etc. These will eventually be replaced by generated types.

After implementing, run `cd ui && pnpm check`.
```

---

### TASK-14: Implement frontend pages and components

**Blocked by:** TASK-13
**Description:** Implement all page routes and business components: MediaCard, ProgressiveImage, VideoPlayer, RuleEditor, admin dashboard, job queue, library browser, login page.

**Prompt template:**
```
You are implementing TASK-14: frontend pages and business components.

Reference: /Users/dallas/git/sceneforged/ui/src/routes/ and /Users/dallas/git/sceneforged/ui/src/lib/components/ for current page and component implementations.

Use Svelte 5 runes exclusively. Use the API client and stores from TASK-13. Use UI primitives from TASK-12.

1. **ProgressiveImage** (src/lib/components/media/ProgressiveImage.svelte): Lazy-loading image with blur-up effect. Loads small thumbnail first (blurred), then full resolution on top with opacity transition.

2. **MediaCard** (src/lib/components/media/MediaCard.svelte): Item card with: ProgressiveImage, profile badges (A/B/AB/C), resolution badges (UHD/FHD/HD), HDR/DV badges, year/runtime/rating. Use $derived for computed values. Navigate on click.

3. **MediaGrid/MediaRow** (src/lib/components/media/): Grid layout and horizontal scrollable row of MediaCards.

4. **VideoPlayer** (src/lib/components/media/VideoPlayer.svelte): HLS video player using hls.js. Play/pause, seek, fullscreen, volume. Reports playback position back.

5. **ErrorBoundary** (src/lib/components/ErrorBoundary.svelte): Catches errors in children, renders fallback with error message and reset button.

6. **Pages**:
   - / (home): Continue watching, recently added, favorites as MediaRows
   - /login: Username/password form with auth store
   - /browse/[libraryId]: MediaGrid with items from library, pagination, search
   - /browse/[libraryId]/[itemId]: Item detail page with metadata, media files, play button
   - /play/[itemId]: Full-screen VideoPlayer
   - /rules: Rule list with enable/disable, RuleEditor for editing
   - /settings: Server config display, conversion config
   - /admin: Dashboard with stats cards, active streams, job queue summary
   - /admin/jobs: Job history table with status, retry, delete actions
   - /admin/libraries: Library list with create/delete, scan trigger
   - /admin/libraries/[libraryId]: Library detail with items and config

After implementing, run `cd ui && pnpm check && pnpm build`.
```

---

### TASK-15: Implement Dockerfile and docker-compose

**Blocked by:** TASK-11, TASK-14
**Description:** Create multi-stage Dockerfile with cargo-chef for dependency caching, and docker-compose.yml for development.

**Prompt template:**
```
You are implementing TASK-15: Docker build and compose setup.

Reference: /Users/dallas/git/sceneforged/Dockerfile for current multi-stage build.

1. **Dockerfile**: 5-stage build:
   - Stage 1 (ui-builder): node:22-alpine, pnpm install, pnpm build, pass PUBLIC_COMMIT_SHA build arg
   - Stage 2 (chef): rust:1-trixie, cargo install cargo-chef, cargo chef prepare
   - Stage 3 (cook): rust:1-trixie, install FFmpeg dev headers (libavformat-dev, libavcodec-dev, libavutil-dev, pkg-config, clang), cargo chef cook --release
   - Stage 4 (builder): copy source, cargo build --release, copy UI build into static dir
   - Stage 5 (runtime): debian:trixie-slim, install ffmpeg mediainfo mkvtoolnix, download dovi_tool from GitHub releases (multi-arch: amd64/arm64), create non-root user, copy binary and static files, EXPOSE 8080, HEALTHCHECK, CMD

2. **docker-compose.yml**: Service for sceneforged with volume mounts for config, data, media. Environment variables for RUST_LOG. Health check.

3. **.dockerignore**: Exclude target/, node_modules/, .git/, etc.

After creating, run `docker build -t sceneforged .` to verify build succeeds.
```

---

### TASK-16: Implement CI/CD pipeline

**Blocked by:** TASK-11, TASK-14
**Description:** Create GitHub Actions workflows for CI (lint, test, build) and CD (Docker build and push).

**Prompt template:**
```
You are implementing TASK-16: GitHub Actions CI/CD.

Create .github/workflows/:

1. **ci.yml** (on push and pull_request):
   - Job "check": cargo fmt --all -- --check, cargo clippy --all-targets --all-features -- -D warnings, cargo test --all --all-features
   - Job "frontend": cd ui && pnpm install --frozen-lockfile && pnpm check && pnpm test && pnpm build
   - Both jobs use caching (cargo registry, target dir, pnpm store)

2. **docker.yml** (on push to main):
   - Needs: ci
   - Build and push Docker image to ghcr.io
   - Use docker/build-push-action with layer caching

3. **release.yml** (on tag push v*):
   - Build release binaries for linux-amd64 and linux-arm64
   - Create GitHub Release with binaries attached

After creating, verify YAML syntax is valid.
```

---

### TASK-17: Integration tests

**Blocked by:** TASK-10, TASK-11
**Description:** Write integration tests that verify the full system: API endpoints, webhook processing, job lifecycle, config management.

**Prompt template:**
```
You are implementing TASK-17: integration tests.

Reference: /Users/dallas/git/sceneforged/tests/ for current test patterns.

Create tests/ directory with:

1. **tests/common/mod.rs**: TestHarness struct that creates an in-memory DB, default config, EventBus, and full AppContext. Method `with_server() -> (Self, SocketAddr)` starts Axum on a random port.

2. **tests/api_test.rs**: Test all API endpoints against TestHarness server:
   - Health check returns 200
   - Auth flow (login, status, logout)
   - Library CRUD
   - Job submission and retrieval
   - Rules get/update
   - Dashboard stats

3. **tests/webhook_test.rs**: Test webhook processing:
   - Radarr webhook parsing and job creation
   - Sonarr webhook parsing and job creation
   - Signature verification (valid and invalid)
   - Unknown arr name returns 404

4. **tests/job_lifecycle_test.rs**: Test job state machine:
   - Queue job -> dequeue -> start -> progress -> complete
   - Queue job -> fail -> retry -> complete
   - Queue job -> fail N times -> dead letter

5. **tests/rule_matching_test.rs**: Test rule engine integration:
   - Simple conditions match
   - OR conditions match
   - NOT conditions exclude
   - Priority ordering
   - Disabled rules skipped

After implementing, run `cargo test --all`.
```

---

## Orchestrator Instructions

You are the orchestrator agent. Your job is to drive the implementation of all tasks to completion. You must:

1. **Create all tasks as todos** using TaskCreate with the blocking relationships defined above.
2. **Start orchestrating immediately** after creating tasks.
3. **You are an orchestrator only** — do NOT implement, test, or check code yourself. Spawn subagents for everything.
4. **For each ready task** (unblocked, pending):
   a. Set it to in_progress
   b. Spawn a task subagent (subagent_type: "general-purpose") with the prompt template filled in
   c. When the subagent returns, spawn a **checker subagent** (model: "haiku") to verify the work
   d. If the checker finds issues, re-spawn the task subagent with the checker's feedback
   e. When the checker approves, mark the task as completed
5. **Run unblocked tasks in parallel** where possible (launch multiple subagents in a single message).
6. **Continue until all tasks are completed.**

### Checker Prompt Template

Use this template when spawning the haiku checker:

```
You are a code reviewer checking the output of TASK-{ID}: {TITLE}.

Check the following:
1. Does the code compile? Run `cargo check` (for Rust) or `pnpm check` (for frontend).
2. Do tests pass? Run `cargo test -p {crate}` or `pnpm test`.
3. Are all files from the task description created?
4. Does the code follow the architectural patterns described (trait-based actions, typed errors, runes-only stores, etc.)?
5. Are there any obvious bugs, missing error handling, or security issues?

If everything passes, respond with "APPROVED" and a brief summary.
If there are issues, respond with "NEEDS_FIXES" followed by a numbered list of specific issues to fix.
```

### Re-run Prompt Template

When the checker returns NEEDS_FIXES, re-spawn the task subagent with:

```
You previously implemented TASK-{ID}: {TITLE}.

The code reviewer found the following issues that need to be fixed:
{CHECKER_FEEDBACK}

Please fix all listed issues. The code is in {DIRECTORY}. Make the minimal changes needed to address each issue.

After fixing, run the relevant check/test commands to verify.
```

### Task Dependency Graph (visual)

```
TASK-01 (workspace init)
  ├── TASK-02 (sf-core) ─────────────────────────────────┐
  │     ├── TASK-03 (sf-probe) ──────────┐               │
  │     │     └── TASK-04 (sf-av) ───────┤               │
  │     │           └────────────────────┤               │
  │     ├── TASK-05 (sf-rules) ──────────┤               │
  │     │     └────────────────────────→ TASK-06 (sf-pipeline)
  │     ├── TASK-07 (sf-db) ─────────────┤               │
  │     └── TASK-08 (sf-parser) ─────────┤               │
  ├── TASK-09 (sf-media) ───────────────┤               │
  │                                      ↓               │
  │                              TASK-10 (sf-server) ←───┘
  │                                      ↓
  │                              TASK-11 (binary) ──→ TASK-15 (Docker)
  │                                      ↓            TASK-16 (CI/CD)
  │                              TASK-17 (tests)
  │
  └── TASK-12 (frontend scaffold)
        └── TASK-13 (stores + API)
              └── TASK-14 (pages + components) ──→ TASK-15 (Docker)
                                                    TASK-16 (CI/CD)
```

### Parallelism Opportunities

After TASK-01 completes:
- TASK-02 and TASK-12 can run in parallel (backend core + frontend scaffold)

After TASK-02 completes:
- TASK-03, TASK-05, TASK-07, TASK-08, TASK-09 can ALL run in parallel

After TASK-03 completes:
- TASK-04 can start

After TASK-04 and TASK-05 complete:
- TASK-06 can start

After TASK-12 completes:
- TASK-13 can start (may overlap with backend tasks)

After TASK-02, TASK-03, TASK-05, TASK-06, TASK-07, TASK-09 complete:
- TASK-10 can start

After TASK-10 completes:
- TASK-11 and TASK-17 can run in parallel

After TASK-13 completes:
- TASK-14 can start

After TASK-11 and TASK-14 complete:
- TASK-15 and TASK-16 can run in parallel
