mod cli;
mod processor;

use sceneforged::{
    config, conversion, pipeline, probe, rules,
    server::{self, auth},
    state, watch,
};
use sceneforged_db::pool::init_pool;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use std::sync::Arc;

async fn start_server(
    host: String,
    port: u16,
    config_path: Option<&std::path::Path>,
) -> Result<()> {
    // Load config
    let mut config = config::load_config_or_default(config_path)?;

    // Override host/port from CLI if specified
    config.server.host = host;
    config.server.port = port;

    tracing::info!("Starting Sceneforged server");
    tracing::info!(
        "Server will listen on {}:{}",
        config.server.host,
        config.server.port
    );

    // Determine data directory from config path or current directory
    let data_dir = config_path
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Initialize database
    let db_path = data_dir.join("sceneforged.db");
    let db_path_str = db_path.to_string_lossy();
    tracing::info!("Initializing database at {}", db_path_str);
    let db_pool = init_pool(&db_path_str)?;

    // Clean up orphaned conversion jobs from previous server session
    if let Ok(conn) = db_pool.get() {
        match sceneforged_db::queries::conversion_jobs::reset_orphaned_jobs(&conn) {
            Ok(count) if count > 0 => {
                tracing::info!("Reset {} orphaned conversion jobs from previous session", count);
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("Failed to reset orphaned conversion jobs: {}", e);
            }
        }
    }

    // Create state
    let state_path = data_dir.join("sceneforged-state.json");
    let state = state::AppState::new(Some(state_path));

    // Create shutdown channel for job processor
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Start job processor
    let processor =
        processor::JobProcessor::new(state.clone(), Arc::new(config.clone()), shutdown_rx);
    let processor_handle = tokio::spawn(processor.run());

    // Start conversion executor
    let converted_dir = data_dir.join("converted");
    let mut profile_b_settings = conversion::ProfileBSettings::default();
    profile_b_settings.output_dir = converted_dir;
    let executor = conversion::ConversionExecutor::with_events(
        db_pool.clone(),
        profile_b_settings,
        state.event_sender(),
    );
    let executor_stop = executor.stop_signal();
    let executor_handle = tokio::task::spawn_blocking(move || {
        if let Err(e) = executor.run() {
            tracing::error!("Conversion executor error: {}", e);
        }
    });

    // Start file watcher if enabled
    let mut watcher = watch::FileWatcher::new(config.watch.clone(), state.clone());
    if config.watch.enabled {
        watcher.start().await?;
    }

    // Start HTTP server with config path and database pool
    let resolved_config_path = config_path.map(|p| p.to_path_buf());
    let server_result =
        server::start_server_with_options(config, state, resolved_config_path, Some(db_pool)).await;

    // Cleanup
    tracing::info!("Shutting down...");
    executor_stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = shutdown_tx.send(()).await;
    let _ = processor_handle.await;
    let _ = executor_handle.await;

    server_result
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    // Respect RUST_LOG env var if set, otherwise use defaults based on verbose flag
    let env_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        if cli.verbose {
            // Verbose mode: trace for sceneforged, debug for HTTP
            "sceneforged=trace,sceneforged_media=trace,sceneforged_db=debug,sceneforged_common=debug,sceneforged_probe=debug,tower_http=debug".to_string()
        } else {
            // Normal mode: debug for sceneforged crates, info for HTTP requests
            "sceneforged=debug,sceneforged_media=debug,sceneforged_db=info,tower_http=info".to_string()
        }
    });

    tracing_subscriber::fmt()
        .with_env_filter(&env_filter)
        .init();

    match cli.command {
        Commands::Start { host, port } => {
            // Create tokio runtime
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(start_server(host, port, cli.config.as_deref()))
        }
        Commands::Run {
            input,
            dry_run,
            force,
        } => run_file(&input, cli.config.as_deref(), dry_run, force),
        Commands::Probe { file, json } => probe_file(&file, json),
        Commands::CheckTools => check_tools(),
        Commands::Validate {
            config: config_path,
        } => {
            let path = config_path.or(cli.config);
            validate_config(path.as_deref())
        }
        Commands::Version => {
            println!("sceneforged {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Commands::HashPassword { password } => hash_password(&password),
        Commands::GenerateApiKey => generate_api_key(),
        Commands::GenerateSecret => generate_secret(),
    }
}

fn run_file(
    input: &std::path::Path,
    config_path: Option<&std::path::Path>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    // Load config
    let config = config::load_config_or_default(config_path)?;

    // Verify input file exists
    if !input.exists() {
        anyhow::bail!("Input file does not exist: {:?}", input);
    }

    tracing::info!("Processing file: {:?}", input);

    // Probe the file
    tracing::info!("Probing media info...");
    let media_info = probe::probe_file(input)?;

    tracing::debug!("Media info: {:?}", media_info);
    println!("File: {}", media_info.file_path.display());
    println!("Container: {}", media_info.container);
    if let Some(video) = media_info.primary_video() {
        println!("Video: {} {}x{}", video.codec, video.width, video.height);
        if let Some(ref hdr) = video.hdr_format {
            println!("HDR: {:?}", hdr);
        }
        if let Some(ref dv) = video.dolby_vision {
            println!("Dolby Vision: Profile {}", dv.profile);
        }
    }

    // Find matching rule
    let matched_rule = rules::find_matching_rule(&media_info, &config.rules);

    match matched_rule {
        Some(rule) => {
            println!("\nMatched rule: {} (priority {})", rule.name, rule.priority);
            println!("Actions to execute: {}", rule.actions.len());

            for (i, action) in rule.actions.iter().enumerate() {
                println!("  {}. {:?}", i + 1, action);
            }

            if dry_run {
                println!("\n[DRY RUN] Would execute {} actions", rule.actions.len());
                return Ok(());
            }

            // Execute the pipeline
            println!("\nExecuting pipeline...");
            let executor = pipeline::PipelineExecutor::new(input, false)?;
            let output = executor.execute(&rule.actions)?;

            println!("\nProcessing complete!");
            println!("Output: {:?}", output);
        }
        None => {
            if force {
                println!("No rules matched, but --force specified. Nothing to do.");
            } else {
                println!("No rules matched for this file.");
                println!("Use --force to process anyway (no actions will be taken).");
            }
        }
    }

    Ok(())
}

fn probe_file(file: &std::path::Path, json: bool) -> Result<()> {
    if !file.exists() {
        anyhow::bail!("File does not exist: {:?}", file);
    }

    let media_info = probe::probe_file(file)?;

    if json {
        let json_str = serde_json::to_string_pretty(&media_info)?;
        println!("{}", json_str);
    } else {
        println!("File: {}", media_info.file_path.display());
        println!("Container: {}", media_info.container);
        println!("Size: {} bytes", media_info.file_size);
        if let Some(ref duration) = media_info.duration {
            let secs = duration.as_secs();
            let mins = secs / 60;
            let hours = mins / 60;
            println!("Duration: {:02}:{:02}:{:02}", hours, mins % 60, secs % 60);
        }

        println!("\nVideo Tracks: {}", media_info.video_tracks.len());
        for (i, track) in media_info.video_tracks.iter().enumerate() {
            println!("  [{}] {} {}x{}", i, track.codec, track.width, track.height);
            if let Some(fps) = track.frame_rate {
                print!("      {:.3} fps", fps);
            }
            if let Some(bits) = track.bit_depth {
                print!(", {} bit", bits);
            }
            println!();
            if let Some(ref hdr) = track.hdr_format {
                println!("      HDR: {:?}", hdr);
            }
            if let Some(ref dv) = track.dolby_vision {
                println!(
                    "      Dolby Vision: Profile {} (RPU: {}, EL: {}, BL: {})",
                    dv.profile, dv.rpu_present, dv.el_present, dv.bl_present
                );
            }
        }

        println!("\nAudio Tracks: {}", media_info.audio_tracks.len());
        for (i, track) in media_info.audio_tracks.iter().enumerate() {
            print!("  [{}] {} {}ch", i, track.codec, track.channels);
            if let Some(ref lang) = track.language {
                print!(" ({})", lang);
            }
            if track.atmos {
                print!(" [Atmos]");
            }
            if track.default {
                print!(" [default]");
            }
            println!();
        }

        println!("\nSubtitle Tracks: {}", media_info.subtitle_tracks.len());
        for (i, track) in media_info.subtitle_tracks.iter().enumerate() {
            print!("  [{}] {}", i, track.codec);
            if let Some(ref lang) = track.language {
                print!(" ({})", lang);
            }
            if track.forced {
                print!(" [forced]");
            }
            if track.default {
                print!(" [default]");
            }
            println!();
        }
    }

    Ok(())
}

fn check_tools() -> Result<()> {
    println!("Checking external tools...\n");

    let tools = probe::check_tools();
    let mut all_ok = true;

    for tool in &tools {
        let status = if tool.available {
            "✓"
        } else {
            all_ok = false;
            "✗"
        };

        print!("{} {}", status, tool.name);

        if let Some(ref version) = tool.version {
            print!(" ({})", version.lines().next().unwrap_or(""));
        }

        if let Some(ref path) = tool.path {
            print!(" - {}", path.display());
        }

        println!();
    }

    println!();
    if all_ok {
        println!("All required tools are available!");
    } else {
        println!("Some tools are missing. Install them to enable all features.");
    }

    Ok(())
}

fn validate_config(path: Option<&std::path::Path>) -> Result<()> {
    match path {
        Some(p) => {
            println!("Validating config: {:?}", p);
            let config = config::load_config(p)?;
            println!("✓ Configuration is valid");
            println!("  Server: {}:{}", config.server.host, config.server.port);
            println!("  Auth enabled: {}", config.server.auth.enabled);
            println!("  Watch enabled: {}", config.watch.enabled);
            println!("  Watch paths: {}", config.watch.paths.len());
            println!("  Arr integrations: {}", config.arrs.len());
            println!("  Rules: {}", config.rules.len());
            println!(
                "    Enabled: {}",
                config.rules.iter().filter(|r| r.enabled).count()
            );
        }
        None => {
            println!("No config file specified, using defaults");
            let config = config::Config::default();
            println!("Default config:");
            println!("  Server: {}:{}", config.server.host, config.server.port);
        }
    }

    Ok(())
}

fn hash_password(password: &str) -> Result<()> {
    let hash = auth::hash_password(password)?;
    println!("{}", hash);
    Ok(())
}

fn generate_api_key() -> Result<()> {
    let key = auth::generate_api_key();
    println!("{}", key);
    Ok(())
}

fn generate_secret() -> Result<()> {
    let secret = auth::generate_secret();
    println!("{}", secret);
    Ok(())
}
