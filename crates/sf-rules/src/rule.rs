//! The [`Rule`] struct combines an expression tree with actions.

use serde::{Deserialize, Serialize};
use sf_core::RuleId;

use crate::action_config::ActionConfig;
use crate::expr::Expr;

/// A processing rule that matches media files and specifies actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Unique identifier for this rule.
    pub id: RuleId,
    /// Human-readable name.
    pub name: String,
    /// Whether this rule is active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Priority (higher values are matched first).
    #[serde(default)]
    pub priority: i32,
    /// Expression tree that must evaluate to `true` for this rule to match.
    pub expr: Expr,
    /// Actions to execute when this rule matches.
    pub actions: Vec<ActionConfig>,
}

fn default_enabled() -> bool {
    true
}
