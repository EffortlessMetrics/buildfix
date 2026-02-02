use camino::Utf8PathBuf;
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FixId(pub String);

impl FixId {
    pub fn new<S: Into<String>>(s: S) -> Self {
        Self(s.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TriggerKey {
    pub tool: String,
    pub check_id: Option<String>,
    pub code: Option<String>,
}

impl TriggerKey {
    pub fn new(tool: impl Into<String>, check_id: Option<String>, code: Option<String>) -> Self {
        Self {
            tool: tool.into(),
            check_id,
            code,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DepPreserve {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_features: Option<bool>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    EnsureWorkspaceResolverV2 {
        manifest: Utf8PathBuf,
    },

    EnsurePathDepHasVersion {
        manifest: Utf8PathBuf,

        /// TOML path to the dependency item, e.g. ["dependencies","foo"] or
        /// ["target","cfg(windows)","dependencies","foo"].
        toml_path: Vec<String>,

        dep: String,
        dep_path: String,
        version: String,
    },

    UseWorkspaceDependency {
        manifest: Utf8PathBuf,

        /// TOML path to the dependency item, e.g. ["dependencies","foo"].
        toml_path: Vec<String>,

        dep: String,
        preserved: DepPreserve,
    },

    SetPackageRustVersion {
        manifest: Utf8PathBuf,
        rust_version: String,
    },
}

impl Operation {
    pub fn manifest(&self) -> &Utf8PathBuf {
        match self {
            Operation::EnsureWorkspaceResolverV2 { manifest }
            | Operation::EnsurePathDepHasVersion { manifest, .. }
            | Operation::UseWorkspaceDependency { manifest, .. }
            | Operation::SetPackageRustVersion { manifest, .. } => manifest,
        }
    }
}
