# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Sceneforged is a media automation platform for post-processing video files with HDR/Dolby Vision support. It combines a Rust backend with a SvelteKit frontend to provide automated media processing through configurable rules.

## Build and Development Commands

### Backend (Rust)
```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo run                      # Start server (default: localhost:8080)
cargo test --all               # Run all tests
cargo test --all --all-features
cargo test -- --ignored        # Integration tests (requires ffmpeg, mediainfo, mkvtoolnix)
cargo fmt --all                # Format code
cargo clippy --all-targets --all-features -- -D warnings
cargo bench --bench probe_parsing    # Run specific benchmark
```

### Frontend (SvelteKit)
```bash
cd ui
pnpm install                   # Install dependencies
pnpm dev                       # Dev server at localhost:5173 (proxies /api to :8080)
pnpm build                     # Production build
pnpm check                     # Type check + svelte-check
pnpm test                      # Unit tests (Vitest)
pnpm test:coverage             # Coverage report
pnpm test:e2e                  # E2E tests (Playwright)
```

### CLI Commands
```bash
sceneforged start [--host 0.0.0.0] [--port 8080] [--config config.toml]
sceneforged run <file> [--dry-run]   # Process single file
sceneforged probe <file> [--json]    # Analyze media file
sceneforged validate                 # Validate config
sceneforged check-tools              # Verify external tools
```

## Architecture

### Workspace Structure
- **`src/`** - Main application (CLI, web server, pipeline executor)
- **`crates/sceneforged-av/`** - Media operations (remux, transcode, conversion)
- **`crates/sceneforged-parser/`** - Release name parsing
- **`crates/sceneforged-probe/`** - Pure Rust video probing (HDR/DV detection without external tools)
- **`ui/`** - SvelteKit 5 frontend

### Processing Pipeline Flow
1. **Probing** (`sceneforged-probe`): Parse container metadata, detect HDR formats (HDR10, HDR10+, HLG, Dolby Vision profiles)
2. **Rule Matching** (`src/rules/`): Evaluate media against TOML-configured rules sorted by priority
3. **Pipeline Execution** (`src/pipeline/`): Apply actions (dv_convert, remux, add_compat_audio, strip_tracks, exec)
4. **State Management** (`src/state/`): Persist job history to JSON

### Web Server Routes (Axum)
- `/api/jobs`, `/api/rules`, `/api/health` - REST API
- `/webhook/{arr_name}` - Radarr/Sonarr integration
- `/events` - SSE for real-time job updates

### Frontend Architecture
- Routes: `ui/src/routes/` (queue, history, rules, settings pages)
- Components: `ui/src/lib/components/ui/` (bits-ui + Tailwind CSS 4)
- Stores: `ui/src/lib/stores/` (Svelte stores for job state, theme)
- Uses Svelte 5 with runes (`$state`, `$derived`, `$effect`)

## External Tool Dependencies

Required for full functionality (searched in PATH or configured in config.toml):
- FFmpeg/ffprobe - Transcoding and analysis
- MediaInfo - Detailed metadata
- MKVMerge - Container remuxing
- dovi_tool - Dolby Vision profile conversion

## Testing Notes

- Rust integration tests marked `#[ignore]` require external tools and media files
- Frontend E2E tests use Playwright with Chromium
- CI runs on GitHub Actions for main/develop branches
- Integration tests with real media only run on manual workflow_dispatch
