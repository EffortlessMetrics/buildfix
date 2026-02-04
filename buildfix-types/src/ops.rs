use serde::{Deserialize, Serialize};

/// Safety class for an operation.
///
/// In buildfix terms:
/// - safe: fully determined from repo-local truth, low impact
/// - guarded: deterministic but higher impact (requires explicit allow)
/// - unsafe: ambiguous without user-provided inputs (plan-only by default)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyClass {
    Safe,
    Guarded,
    Unsafe,
}

impl SafetyClass {
    pub fn is_safe(self) -> bool {
        matches!(self, SafetyClass::Safe)
    }
    pub fn is_guarded(self) -> bool {
        matches!(self, SafetyClass::Guarded)
    }
    pub fn is_unsafe(self) -> bool {
        matches!(self, SafetyClass::Unsafe)
    }
}

/// Operation kind for plan ops.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpKind {
    TomlSet {
        toml_path: Vec<String>,
        value: serde_json::Value,
    },
    TomlRemove {
        toml_path: Vec<String>,
    },
    TomlTransform {
        rule_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
    },
}

/// Target path for an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpTarget {
    pub path: String,
}

/// Optional preview fragment for an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpPreview {
    pub patch_fragment: String,
}
