pub mod persist;
mod types;

pub use types::*;

use anyhow::{Context, Result};
use std::path::Path;

/// Load configuration from a TOML file
pub fn load_config(path: &Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let mut config: Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {:?}", path))?;

    validate_config(&config)?;

    prepare_rules(&mut config.rules);

    Ok(config)
}

/// Load config from default locations or return default config
pub fn load_config_or_default(custom_path: Option<&Path>) -> Result<Config> {
    if let Some(path) = custom_path {
        return load_config(path);
    }

    // Try default locations
    let default_paths = [
        "./config.toml",
        "./sceneforged.toml",
        "~/.config/sceneforged/config.toml",
        "/etc/sceneforged/config.toml",
    ];

    for path_str in default_paths {
        let path = shellexpand::tilde(path_str);
        let path = Path::new(path.as_ref());
        if path.exists() {
            return load_config(path);
        }
    }

    // Return default config if no file found
    let mut config = Config::default();
    prepare_rules(&mut config.rules);
    Ok(config)
}

fn prepare_rules(rules: &mut [Rule]) {
    rules.sort_by(|a, b| b.priority.cmp(&a.priority));

    for rule in rules.iter_mut() {
        rule.normalized = Some(NormalizedMatchConditions::from(&rule.match_conditions));
    }
}

/// Validate configuration
fn validate_config(config: &Config) -> Result<()> {
    // Validate server config
    if config.server.port == 0 {
        anyhow::bail!("Server port cannot be 0");
    }

    // Validate watch paths exist
    for path in &config.watch.paths {
        if !path.exists() {
            tracing::warn!("Watch path does not exist: {:?}", path);
        }
    }

    // Validate arr configs
    for arr in &config.arrs {
        if arr.enabled && arr.api_key.is_empty() {
            anyhow::bail!("Arr '{}' is enabled but has no API key", arr.name);
        }
    }

    // Validate rules
    for rule in &config.rules {
        if rule.enabled && rule.actions.is_empty() {
            anyhow::bail!("Rule '{}' is enabled but has no actions", rule.name);
        }
    }

    Ok(())
}
