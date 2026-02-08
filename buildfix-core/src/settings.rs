//! Clap-free settings for plan and apply pipelines.

use camino::Utf8PathBuf;
use std::collections::HashMap;

/// Run mode controls exit-code semantics.
///
/// In `Cockpit` mode, policy blocks (exit 2) are mapped to exit 0
/// because the receipt still encodes the block in its verdict/data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RunMode {
    #[default]
    Standalone,
    Cockpit,
}

/// Settings for the plan pipeline.
#[derive(Debug, Clone)]
pub struct PlanSettings {
    pub repo_root: Utf8PathBuf,
    pub artifacts_dir: Utf8PathBuf,
    pub out_dir: Utf8PathBuf,

    // Policy
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub allow_guarded: bool,
    pub allow_unsafe: bool,
    pub allow_dirty: bool,
    pub max_ops: Option<u64>,
    pub max_files: Option<u64>,
    pub max_patch_bytes: Option<u64>,
    pub params: HashMap<String, String>,

    // Preconditions
    pub require_clean_hashes: bool,
    pub git_head_precondition: bool,

    // Backups
    pub backup_suffix: String,

    // Mode
    pub mode: RunMode,
}

impl Default for PlanSettings {
    fn default() -> Self {
        Self {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            out_dir: Utf8PathBuf::from("artifacts/buildfix"),
            allow: Vec::new(),
            deny: Vec::new(),
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: None,
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
            require_clean_hashes: true,
            git_head_precondition: false,
            backup_suffix: ".buildfix.bak".to_string(),
            mode: RunMode::default(),
        }
    }
}

/// Settings for the apply pipeline.
#[derive(Debug, Clone)]
pub struct ApplySettings {
    pub repo_root: Utf8PathBuf,
    pub out_dir: Utf8PathBuf,

    // Apply behaviour
    pub dry_run: bool,
    pub allow_guarded: bool,
    pub allow_unsafe: bool,
    pub allow_dirty: bool,
    pub params: HashMap<String, String>,

    // Backups
    pub backup_enabled: bool,
    pub backup_suffix: String,

    // Mode
    pub mode: RunMode,
}

impl Default for ApplySettings {
    fn default() -> Self {
        Self {
            repo_root: Utf8PathBuf::from("."),
            out_dir: Utf8PathBuf::from("artifacts/buildfix"),
            dry_run: true,
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            params: HashMap::new(),
            backup_enabled: true,
            backup_suffix: ".buildfix.bak".to_string(),
            mode: RunMode::default(),
        }
    }
}
