//! Configuration persistence using toml_edit to preserve formatting and comments.

use super::{ArrConfig, JellyfinConfig, Rule};
use anyhow::{Context, Result};
use std::path::Path;
use toml_edit::DocumentMut;

/// Save the entire config to a TOML file, preserving existing structure
pub fn save_config(path: &Path, config: &super::Config) -> Result<()> {
    // Convert config to TOML string and parse as document
    let new_content =
        toml::to_string_pretty(config).with_context(|| "Failed to serialize config")?;
    let new_doc: DocumentMut = new_content
        .parse()
        .with_context(|| "Failed to parse serialized config")?;

    // For now, just write the new config (full replacement)
    // A more sophisticated implementation would merge changes
    std::fs::write(path, new_doc.to_string())
        .with_context(|| format!("Failed to write config file: {:?}", path))?;

    Ok(())
}

/// Update just the rules section of the config file
pub fn update_rules(path: &Path, rules: &[Rule]) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let mut doc: DocumentMut = content
        .parse()
        .with_context(|| format!("Failed to parse config file: {:?}", path))?;

    // Serialize rules to TOML
    let rules_toml = toml::to_string(&RulesWrapper {
        rules: rules.to_vec(),
    })
    .with_context(|| "Failed to serialize rules")?;
    let rules_doc: DocumentMut = rules_toml
        .parse()
        .with_context(|| "Failed to parse serialized rules")?;

    // Replace the rules array
    if let Some(rules_item) = rules_doc.get("rules") {
        doc["rules"] = rules_item.clone();
    } else {
        doc.remove("rules");
    }

    std::fs::write(path, doc.to_string())
        .with_context(|| format!("Failed to write config file: {:?}", path))?;

    Ok(())
}

/// Update just the arrs section of the config file
pub fn update_arrs(path: &Path, arrs: &[ArrConfig]) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let mut doc: DocumentMut = content
        .parse()
        .with_context(|| format!("Failed to parse config file: {:?}", path))?;

    // Serialize arrs to TOML
    let arrs_toml = toml::to_string(&ArrsWrapper {
        arrs: arrs.to_vec(),
    })
    .with_context(|| "Failed to serialize arrs")?;
    let arrs_doc: DocumentMut = arrs_toml
        .parse()
        .with_context(|| "Failed to parse serialized arrs")?;

    // Replace the arrs array
    if let Some(arrs_item) = arrs_doc.get("arrs") {
        doc["arrs"] = arrs_item.clone();
    } else {
        doc.remove("arrs");
    }

    std::fs::write(path, doc.to_string())
        .with_context(|| format!("Failed to write config file: {:?}", path))?;

    Ok(())
}

/// Update just the jellyfins section of the config file
pub fn update_jellyfins(path: &Path, jellyfins: &[JellyfinConfig]) -> Result<()> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let mut doc: DocumentMut = content
        .parse()
        .with_context(|| format!("Failed to parse config file: {:?}", path))?;

    // Serialize jellyfins to TOML
    let jellyfins_toml = toml::to_string(&JellyfinsWrapper {
        jellyfins: jellyfins.to_vec(),
    })
    .with_context(|| "Failed to serialize jellyfins")?;
    let jellyfins_doc: DocumentMut = jellyfins_toml
        .parse()
        .with_context(|| "Failed to parse serialized jellyfins")?;

    // Replace the jellyfins array
    if let Some(jellyfins_item) = jellyfins_doc.get("jellyfins") {
        doc["jellyfins"] = jellyfins_item.clone();
    } else {
        doc.remove("jellyfins");
    }

    std::fs::write(path, doc.to_string())
        .with_context(|| format!("Failed to write config file: {:?}", path))?;

    Ok(())
}

// Wrapper structs for serialization
#[derive(serde::Serialize)]
struct RulesWrapper {
    rules: Vec<Rule>,
}

#[derive(serde::Serialize)]
struct ArrsWrapper {
    arrs: Vec<ArrConfig>,
}

#[derive(serde::Serialize)]
struct JellyfinsWrapper {
    jellyfins: Vec<JellyfinConfig>,
}
