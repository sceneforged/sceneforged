//! # sf-rules
//!
//! Encoding rules and validation logic for media processing.
//!
//! This crate provides a composable rule engine that matches media files
//! based on their properties (codec, container, HDR format, resolution, etc.)
//! and specifies actions to take when rules match.
//!
//! ## Overview
//!
//! - [`Condition`] -- leaf conditions that test a single media property.
//! - [`Expr`] -- expression tree combining conditions with AND/OR/NOT.
//! - [`ActionConfig`] -- what to do when a rule matches.
//! - [`Rule`] -- binds an expression to a set of actions with priority.
//! - [`RuleEngine`] -- evaluates media files against a sorted set of rules.

pub mod action_config;
pub mod condition;
pub mod engine;
pub mod expr;
pub mod rule;

pub use action_config::ActionConfig;
pub use condition::Condition;
pub use engine::RuleEngine;
pub use expr::{evaluate, Expr};
pub use rule::Rule;

/// Serialize a list of rules to a JSON string.
///
/// This function is provided so that downstream crates can serialize rules
/// without triggering deep generic monomorphization of serde's `Serializer`
/// trait for the recursive [`Expr`] type (which can cause compiler ICE or
/// require very high recursion limits).
pub fn serialize_rules(rules: &[Rule]) -> Result<String, serde_json::Error> {
    serde_json::to_string(rules)
}

/// Serialize a list of rules to a pretty-printed JSON string.
pub fn serialize_rules_pretty(rules: &[Rule]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(rules)
}

/// Deserialize a list of rules from a JSON string.
///
/// Counterpart to [`serialize_rules`]; keeps the serde monomorphization
/// inside this crate.
pub fn deserialize_rules(json: &str) -> Result<Vec<Rule>, serde_json::Error> {
    serde_json::from_str(json)
}

/// Serialize rules to a [`serde_json::Value`].
///
/// Serializes via string to keep monomorphization in this crate.
pub fn rules_to_value(rules: &[Rule]) -> Result<serde_json::Value, serde_json::Error> {
    let s = serde_json::to_string(rules)?;
    serde_json::from_str(&s)
}

/// Deserialize rules from a [`serde_json::Value`].
///
/// Serializes the value to string first to keep monomorphization in this crate.
pub fn rules_from_value(value: &serde_json::Value) -> Result<Vec<Rule>, serde_json::Error> {
    let s = serde_json::to_string(value)?;
    serde_json::from_str(&s)
}
