//! Service-oriented application context.
//!
//! [`AppContext`] is the central struct shared across all route handlers via
//! Axum state. It wraps immutable infrastructure (DB pool, tools) in `Arc`s
//! and mutable runtime configuration in a [`ConfigStore`] with hot-reload
//! support.

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::RwLock;

use sf_av::ToolRegistry;
use sf_core::config::Config;
use sf_core::events::EventBus;
use sf_core::MediaFileId;
use sf_db::pool::DbPool;
use sf_media::PreparedMedia;
use sf_probe::Prober;
use sf_rules::Rule;

// ---------------------------------------------------------------------------
// ConfigStore
// ---------------------------------------------------------------------------

/// Mutable runtime configuration that can be updated via API and persisted.
///
/// All fields are behind [`RwLock`] so readers never block each other and
/// writes are short-lived.
#[derive(Debug)]
pub struct ConfigStore {
    /// Processing rules (editable via PUT /api/config/rules).
    pub rules: RwLock<Vec<Rule>>,
    /// Arr (Radarr/Sonarr) integration configs.
    pub arrs: RwLock<Vec<sf_core::config::ArrConfig>>,
    /// Jellyfin server configs.
    pub jellyfins: RwLock<Vec<sf_core::config::JellyfinConfig>>,
    /// Conversion defaults.
    pub conversion: RwLock<sf_core::config::ConversionConfig>,
    /// Path to the config file for persistence (None = no persistence).
    config_path: Option<PathBuf>,
}

impl ConfigStore {
    /// Build a new store from the given config and optional file path.
    pub fn new(config: &Config, config_path: Option<PathBuf>) -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
            arrs: RwLock::new(config.arrs.clone()),
            jellyfins: RwLock::new(config.jellyfins.clone()),
            conversion: RwLock::new(config.conversion.clone()),
            config_path,
        }
    }

    /// Replace the rules.
    pub fn set_rules(&self, rules: Vec<Rule>) {
        *self.rules.write() = rules;
    }

    /// Read a snapshot of the current rules.
    pub fn get_rules(&self) -> Vec<Rule> {
        self.rules.read().clone()
    }

    /// Persist the current mutable config back to the file.
    ///
    /// This is a best-effort operation; errors are logged but not propagated.
    pub fn persist(&self) {
        let Some(ref path) = self.config_path else {
            return;
        };

        let rules = self.get_rules();
        let arrs = self.arrs.read().clone();
        let jellyfins = self.jellyfins.read().clone();
        let conversion = self.conversion.read().clone();

        // Use sf_rules::rules_to_value for rules serialization to keep the
        // deep serde monomorphization for the recursive Expr type inside the
        // sf-rules crate. Other config sections use serde_json::to_value
        // directly since they don't have recursive types.
        let mut map = serde_json::Map::new();
        if let Ok(v) = sf_rules::rules_to_value(&rules) {
            map.insert("rules".into(), v);
        }
        if let Ok(v) = serde_json::to_value(&arrs) {
            map.insert("arrs".into(), v);
        }
        if let Ok(v) = serde_json::to_value(&jellyfins) {
            map.insert("jellyfins".into(), v);
        }
        if let Ok(v) = serde_json::to_value(&conversion) {
            map.insert("conversion".into(), v);
        }

        let snapshot = serde_json::Value::Object(map);

        match serde_json::to_string_pretty(&snapshot) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path, json) {
                    tracing::warn!("Failed to persist config to {}: {e}", path.display());
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize config: {e}");
            }
        }
    }

    /// Reload mutable config from the file on disk.
    pub fn reload(&self) {
        let Some(ref path) = self.config_path else {
            return;
        };

        let contents = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to read config for reload from {}: {e}", path.display());
                return;
            }
        };

        // Try to parse as our JSON snapshot format.
        // Use sf_rules::rules_from_value for rules to keep the deep serde
        // monomorphization for Expr inside the sf-rules crate.
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&contents) {
            if let Some(rules) = val.get("rules") {
                if let Ok(r) = sf_rules::rules_from_value(rules) {
                    self.set_rules(r);
                }
            }
            if let Some(arrs) = val.get("arrs") {
                if let Ok(a) =
                    serde_json::from_value::<Vec<sf_core::config::ArrConfig>>(arrs.clone())
                {
                    *self.arrs.write() = a;
                }
            }
            if let Some(jfs) = val.get("jellyfins") {
                if let Ok(j) =
                    serde_json::from_value::<Vec<sf_core::config::JellyfinConfig>>(jfs.clone())
                {
                    *self.jellyfins.write() = j;
                }
            }
            if let Some(conv) = val.get("conversion") {
                if let Ok(c) =
                    serde_json::from_value::<sf_core::config::ConversionConfig>(conv.clone())
                {
                    *self.conversion.write() = c;
                }
            }

            tracing::info!("Config reloaded from {}", path.display());
        }
    }
}

// ---------------------------------------------------------------------------
// AppContext
// ---------------------------------------------------------------------------

/// Application context shared by all request handlers (via Axum state).
///
/// This is cheaply cloneable because it only holds `Arc`s.
#[derive(Clone)]
pub struct AppContext {
    /// Database connection pool.
    pub db: DbPool,
    /// Immutable application configuration snapshot.
    pub config: Arc<Config>,
    /// Mutable runtime configuration with hot-reload.
    pub config_store: Arc<ConfigStore>,
    /// Broadcast event bus for SSE.
    pub event_bus: Arc<EventBus>,
    /// Media file prober.
    pub prober: Arc<dyn Prober>,
    /// External tool registry.
    pub tools: Arc<ToolRegistry>,
    /// In-memory HLS segment cache for zero-copy serving.
    pub hls_cache: Arc<DashMap<MediaFileId, Arc<PreparedMedia>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_store_rules_round_trip() {
        let config = Config::default();
        let store = ConfigStore::new(&config, None);

        assert!(store.get_rules().is_empty());

        let rule = sf_rules::Rule {
            id: sf_core::RuleId::new(),
            name: "test".into(),
            enabled: true,
            priority: 10,
            expr: sf_rules::Expr::And(vec![]),
            actions: vec![],
        };
        store.set_rules(vec![rule.clone()]);

        let rules = store.get_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "test");
    }

    #[test]
    fn config_store_persist_no_path() {
        let config = Config::default();
        let store = ConfigStore::new(&config, None);
        // Should not panic when there is no path.
        store.persist();
    }
}
