mod cli;

use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use cli::{Cli, Commands};
use sf_core::config::Config;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize tracing. Respect RUST_LOG env var; otherwise use defaults
    // based on the verbose flag.
    let env_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        if cli.verbose {
            "sceneforged=trace,sf_server=trace,sf_core=debug,sf_probe=debug,sf_av=debug,sf_rules=debug,sf_pipeline=debug,tower_http=debug".to_string()
        } else {
            "sceneforged=debug,sf_server=info,tower_http=info".to_string()
        }
    });

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&env_filter))
        .init();

    match cli.command {
        Commands::Start { host, port } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(start_server(host, port, cli.config.as_deref()))
        }
        Commands::Run {
            input,
            dry_run,
            force,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(run_file(&input, cli.config.as_deref(), dry_run, force))
        }
        Commands::Probe { file, json } => probe_file(&file, json),
        Commands::CheckTools => check_tools(cli.config.as_deref()),
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

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

async fn start_server(
    host: String,
    port: u16,
    config_path: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load_or_default(config_path);

    // Override host/port from CLI flags.
    config.server.host = host;
    config.server.port = port;

    tracing::info!("Starting sceneforged server");
    tracing::info!(
        "Server will listen on {}:{}",
        config.server.host,
        config.server.port
    );

    sf_server::start(config, config_path.map(|p| p.to_path_buf())).await?;
    Ok(())
}

async fn run_file(
    input: &Path,
    config_path: Option<&Path>,
    dry_run: bool,
    force: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load_or_default(config_path);

    if !input.exists() {
        return Err(format!("Input file does not exist: {}", input.display()).into());
    }

    tracing::info!("Processing file: {}", input.display());

    // Probe the file.
    tracing::info!("Probing media info...");
    let prober = sf_probe::CompositeProber::new(vec![Box::new(sf_probe::RustProber::new())]);
    let media_info = sf_probe::Prober::probe(&prober, input)?;

    tracing::debug!("Media info: {:?}", media_info);
    println!("File: {}", media_info.file_path.display());
    println!("Container: {}", media_info.container);
    if let Some(video) = media_info.primary_video() {
        println!("Video: {} {}x{}", video.codec, video.width, video.height);
        if video.hdr_format != sf_core::HdrFormat::Sdr {
            println!("HDR: {:?}", video.hdr_format);
        }
        if let Some(ref dv) = video.dolby_vision {
            println!("Dolby Vision: Profile {}", dv.profile);
        }
    }

    // Find matching rules.
    let engine = sf_rules::RuleEngine::new(vec![]); // No rules in default config yet
    let matched_rule = engine.find_matching_rule(&media_info);

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

            // Set up tools and workspace.
            let tools = Arc::new(sf_av::ToolRegistry::discover(&config.tools));
            let actions = sf_pipeline::create_actions(&rule.actions, &tools)?;
            let workspace = Arc::new(sf_av::Workspace::new(input)?);
            let ctx = sf_pipeline::ActionContext::new(
                workspace,
                Arc::new(media_info),
                tools,
            )
            .with_dry_run(dry_run);

            let executor = sf_pipeline::PipelineExecutor::new(actions);
            let output = executor.execute(&ctx).await?;

            println!("\nProcessing complete!");
            println!("Output: {}", output.display());
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

fn probe_file(file: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if !file.exists() {
        return Err(format!("File does not exist: {}", file.display()).into());
    }

    let prober = sf_probe::CompositeProber::new(vec![Box::new(sf_probe::RustProber::new())]);
    let media_info = sf_probe::Prober::probe(&prober, file)?;

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
            if track.hdr_format != sf_core::HdrFormat::Sdr {
                println!("      HDR: {:?}", track.hdr_format);
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

fn check_tools(config_path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load_or_default(config_path);
    let registry = sf_av::ToolRegistry::discover(&config.tools);
    let tools = registry.check_all();

    println!("Checking external tools...\n");

    let mut all_ok = true;
    for tool in &tools {
        let status = if tool.available { "OK" } else { all_ok = false; "MISSING" };

        print!("[{:>7}] {}", status, tool.name);

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

fn validate_config(path: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    match path {
        Some(p) => {
            println!("Validating config: {}", p.display());
            let contents = std::fs::read_to_string(p)?;
            let config = Config::from_toml(&contents)?;

            let warnings = config.validate();
            if warnings.is_empty() {
                println!("Configuration is valid");
            } else {
                for w in &warnings {
                    println!("  Warning: {}", w);
                }
            }

            println!("  Server: {}:{}", config.server.host, config.server.port);
            println!("  Auth enabled: {}", config.auth.enabled);
            println!("  Watch enabled: {}", config.watch.enabled);
            println!("  Watch paths: {}", config.watch.paths.len());
            println!("  Arr integrations: {}", config.arrs.len());
            println!(
                "  HW accel: {}",
                config.conversion.hw_accel.as_deref().unwrap_or("none (software)")
            );
        }
        None => {
            println!("No config file specified, using defaults");
            let config = Config::default();
            println!("Default config:");
            println!("  Server: {}:{}", config.server.host, config.server.port);
        }
    }

    Ok(())
}

fn hash_password(password: &str) -> Result<(), Box<dyn std::error::Error>> {
    let hash = bcrypt::hash(password, 12)?;
    println!("{hash}");
    Ok(())
}

fn generate_api_key() -> Result<(), Box<dyn std::error::Error>> {
    use rand::Rng;
    let mut buf = [0u8; 32];
    rand::thread_rng().fill(&mut buf);
    let hex_str: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    println!("sf-{hex_str}");
    Ok(())
}

fn generate_secret() -> Result<(), Box<dyn std::error::Error>> {
    use rand::Rng;
    let mut buf = [0u8; 64];
    rand::thread_rng().fill(&mut buf);
    let hex_str: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    println!("{hex_str}");
    Ok(())
}
