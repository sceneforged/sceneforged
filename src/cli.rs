use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sceneforged")]
#[command(author, version, about = "Media post-processing automation tool")]
pub struct Cli {
    /// Path to config file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the server with web UI and webhook receiver
    Start {
        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: String,

        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },

    /// Process a single file through the pipeline
    Run {
        /// Input file to process
        #[arg(required = true)]
        input: PathBuf,

        /// Show what would be done without executing
        #[arg(long)]
        dry_run: bool,

        /// Force processing even if no rules match
        #[arg(long)]
        force: bool,
    },

    /// Probe a media file and display information
    Probe {
        /// File to probe
        #[arg(required = true)]
        file: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Check that required external tools are available
    CheckTools,

    /// Validate configuration file
    Validate {
        /// Config file to validate (uses default if not specified)
        config: Option<PathBuf>,
    },

    /// Display version information
    Version,

    /// Generate a bcrypt password hash for authentication
    HashPassword {
        /// Password to hash
        password: String,
    },

    /// Generate a random API key for programmatic access
    GenerateApiKey,

    /// Generate a random secret for webhook signature verification
    GenerateSecret,
}
