//! Configuration file loading for buildfix.
//!
//! Discovers and loads `buildfix.toml` from the repository root.
//! Merges config file settings with CLI arguments (CLI takes precedence).

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::debug;

/// The config file name to search for.
pub const CONFIG_FILE_NAME: &str = "buildfix.toml";

/// Top-level configuration from buildfix.toml.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BuildfixConfig {
    /// Policy settings (allow/deny lists, safety, caps).
    pub policy: PolicyConfig,

    /// Backup settings.
    pub backups: BackupsConfig,

    /// Parameters for unsafe fixes.
    pub params: HashMap<String, String>,
}

/// Policy section of the config.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PolicyConfig {
    /// Allowlist patterns for policy keys.
    /// If non-empty, only allowlisted policy keys are eligible.
    pub allow: Vec<String>,

    /// Denylist patterns for policy keys.
    pub deny: Vec<String>,

    /// Allow guarded fixes to run.
    pub allow_guarded: bool,

    /// Allow unsafe fixes to run.
    pub allow_unsafe: bool,

    /// Allow applying fixes when working directory is dirty.
    pub allow_dirty: bool,

    /// Maximum number of operations allowed.
    pub max_ops: Option<u64>,

    /// Maximum number of files allowed to be modified.
    pub max_files: Option<u64>,

    /// Maximum size of the patch in bytes.
    pub max_patch_bytes: Option<u64>,
}

/// Backups section of the config.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BackupsConfig {
    /// Whether to create backups before applying changes.
    pub enabled: bool,

    /// Suffix for backup files.
    pub suffix: String,
}

impl Default for BackupsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            suffix: ".buildfix.bak".to_string(),
        }
    }
}

/// Discover the buildfix.toml config file.
///
/// Searches for `buildfix.toml` in the repository root directory.
/// Returns `None` if no config file is found.
pub fn discover_config(repo_root: &Utf8Path) -> Option<Utf8PathBuf> {
    let config_path = repo_root.join(CONFIG_FILE_NAME);
    if config_path.exists() {
        debug!("found config file at {}", config_path);
        Some(config_path)
    } else {
        debug!("no config file found at {}", config_path);
        None
    }
}

/// Load and parse a buildfix.toml config file.
///
/// Returns an error if the file cannot be read or parsed.
pub fn load_config(path: &Utf8Path) -> anyhow::Result<BuildfixConfig> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("read config file {}", path))?;
    parse_config(&contents).with_context(|| format!("parse config file {}", path))
}

/// Parse a config file from a string.
pub fn parse_config(contents: &str) -> anyhow::Result<BuildfixConfig> {
    let config: BuildfixConfig = toml::from_str(contents).context("invalid TOML")?;
    Ok(config)
}

/// Load config from repo root, or return default if not found.
pub fn load_or_default(repo_root: &Utf8Path) -> anyhow::Result<BuildfixConfig> {
    match discover_config(repo_root) {
        Some(path) => load_config(&path),
        None => Ok(BuildfixConfig::default()),
    }
}

/// Merged configuration combining config file and CLI arguments.
///
/// CLI arguments take precedence over config file settings.
#[derive(Debug, Clone, Default)]
pub struct MergedConfig {
    /// Allow patterns (from config file, extended by CLI).
    pub allow: Vec<String>,

    /// Deny patterns (from config file, extended by CLI).
    pub deny: Vec<String>,

    /// Whether to allow guarded fixes.
    pub allow_guarded: bool,

    /// Whether to allow unsafe fixes.
    pub allow_unsafe: bool,

    /// Whether to allow applying when dirty.
    pub allow_dirty: bool,

    /// Whether to require clean hashes for preconditions.
    pub require_clean_hashes: bool,

    /// Maximum number of operations (from config).
    pub max_ops: Option<u64>,

    /// Maximum number of files (from config).
    pub max_files: Option<u64>,

    /// Maximum patch size in bytes (from config).
    pub max_patch_bytes: Option<u64>,

    /// Backup settings.
    pub backups: BackupsConfig,

    /// Parameters for unsafe fixes.
    pub params: HashMap<String, String>,
}

/// Builder for merging config file with CLI arguments.
pub struct ConfigMerger {
    config: BuildfixConfig,
}

impl ConfigMerger {
    /// Create a new merger from a loaded config.
    pub fn new(config: BuildfixConfig) -> Self {
        Self { config }
    }

    /// Merge with plan command CLI arguments.
    ///
    /// CLI `allow` and `deny` lists extend the config file lists.
    /// CLI `no_clean_hashes` overrides the default behavior.
    pub fn merge_plan_args(
        self,
        cli_allow: &[String],
        cli_deny: &[String],
        no_clean_hashes: bool,
        cli_params: &HashMap<String, String>,
    ) -> MergedConfig {
        let mut allow = self.config.policy.allow.clone();
        let mut deny = self.config.policy.deny.clone();

        // CLI extends the config file lists
        for pattern in cli_allow {
            if !allow.contains(pattern) {
                allow.push(pattern.clone());
            }
        }
        for pattern in cli_deny {
            if !deny.contains(pattern) {
                deny.push(pattern.clone());
            }
        }

        let mut params = self.config.params.clone();
        for (k, v) in cli_params {
            params.insert(k.clone(), v.clone());
        }

        MergedConfig {
            allow,
            deny,
            allow_guarded: self.config.policy.allow_guarded,
            allow_unsafe: self.config.policy.allow_unsafe,
            allow_dirty: self.config.policy.allow_dirty,
            require_clean_hashes: !no_clean_hashes,
            max_ops: self.config.policy.max_ops,
            max_files: self.config.policy.max_files,
            max_patch_bytes: self.config.policy.max_patch_bytes,
            backups: self.config.backups.clone(),
            params,
        }
    }

    /// Merge with apply command CLI arguments.
    ///
    /// CLI boolean flags override config file settings when explicitly set.
    pub fn merge_apply_args(
        self,
        cli_allow_guarded: bool,
        cli_allow_unsafe: bool,
        cli_params: &HashMap<String, String>,
    ) -> MergedConfig {
        // CLI flags override config when set to true
        let allow_guarded = cli_allow_guarded || self.config.policy.allow_guarded;
        let allow_unsafe = cli_allow_unsafe || self.config.policy.allow_unsafe;

        let mut params = self.config.params.clone();
        for (k, v) in cli_params {
            params.insert(k.clone(), v.clone());
        }

        MergedConfig {
            allow: self.config.policy.allow.clone(),
            deny: self.config.policy.deny.clone(),
            allow_guarded,
            allow_unsafe,
            allow_dirty: self.config.policy.allow_dirty,
            require_clean_hashes: true,
            max_ops: self.config.policy.max_ops,
            max_files: self.config.policy.max_files,
            max_patch_bytes: self.config.policy.max_patch_bytes,
            backups: self.config.backups.clone(),
            params,
        }
    }
}

/// Parse CLI params from key=value strings.
pub fn parse_cli_params(params: &[String]) -> anyhow::Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    for entry in params {
        let mut parts = entry.splitn(2, '=');
        let key = parts
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("invalid param '{}': missing key", entry))?;
        let value = parts
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("invalid param '{}': missing value", entry))?;
        out.insert(key.to_string(), value.to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_example_config() {
        let contents = r#"
[policy]
allow = [
  "builddiag/workspace.resolver_v2/*",
  "depguard/deps.path_requires_version/*",
]
deny = []
allow_guarded = false
allow_unsafe = false
allow_dirty = false
max_ops = 50
max_files = 25
max_patch_bytes = 250000

[backups]
enabled = true
suffix = ".buildfix.bak"

[params]
# rust_version = "1.75"
"#;

        let config = parse_config(contents).unwrap();
        assert_eq!(config.policy.allow.len(), 2);
        assert!(!config.policy.allow_guarded);
        assert!(!config.policy.allow_unsafe);
        assert_eq!(config.policy.max_ops, Some(50));
        assert_eq!(config.policy.max_files, Some(25));
        assert_eq!(config.policy.max_patch_bytes, Some(250000));
        assert!(config.backups.enabled);
        assert_eq!(config.backups.suffix, ".buildfix.bak");
    }

    #[test]
    fn test_parse_minimal_config() {
        let contents = r#"
[policy]
allow = ["some/pattern/*"]
"#;

        let config = parse_config(contents).unwrap();
        assert_eq!(config.policy.allow, vec!["some/pattern/*"]);
        assert!(config.policy.deny.is_empty());
        // Defaults
        assert!(!config.policy.allow_guarded);
        assert!(!config.policy.allow_unsafe);
        assert!(config.backups.enabled);
    }

    #[test]
    fn test_parse_empty_config() {
        let contents = "";
        let config = parse_config(contents).unwrap();
        assert!(config.policy.allow.is_empty());
        assert!(config.policy.deny.is_empty());
    }

    #[test]
    fn test_merge_plan_args_cli_extends() {
        let config = BuildfixConfig {
            policy: PolicyConfig {
                allow: vec!["config/pattern/*".to_string()],
                deny: vec!["config/deny/*".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        let cli_allow = vec!["cli/pattern/*".to_string()];
        let cli_deny = vec!["cli/deny/*".to_string()];

        let merged = ConfigMerger::new(config).merge_plan_args(
            &cli_allow,
            &cli_deny,
            false,
            &HashMap::new(),
        );

        assert_eq!(merged.allow.len(), 2);
        assert!(merged.allow.contains(&"config/pattern/*".to_string()));
        assert!(merged.allow.contains(&"cli/pattern/*".to_string()));
        assert_eq!(merged.deny.len(), 2);
        assert!(merged.deny.contains(&"config/deny/*".to_string()));
        assert!(merged.deny.contains(&"cli/deny/*".to_string()));
        assert!(merged.require_clean_hashes);
    }

    #[test]
    fn test_merge_plan_args_no_clean_hashes() {
        let config = BuildfixConfig::default();
        let merged = ConfigMerger::new(config).merge_plan_args(&[], &[], true, &HashMap::new());

        assert!(!merged.require_clean_hashes);
    }

    #[test]
    fn test_merge_apply_args_cli_overrides() {
        let config = BuildfixConfig {
            policy: PolicyConfig {
                allow_guarded: false,
                allow_unsafe: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let merged = ConfigMerger::new(config).merge_apply_args(true, true, &HashMap::new());

        assert!(merged.allow_guarded);
        assert!(merged.allow_unsafe);
    }

    #[test]
    fn test_merge_apply_args_config_used_when_cli_false() {
        let config = BuildfixConfig {
            policy: PolicyConfig {
                allow_guarded: true,
                allow_unsafe: true,
                ..Default::default()
            },
            ..Default::default()
        };

        // CLI flags are false, but config has true
        let merged = ConfigMerger::new(config).merge_apply_args(false, false, &HashMap::new());

        // Config values should be used
        assert!(merged.allow_guarded);
        assert!(merged.allow_unsafe);
    }

    #[test]
    fn test_params_preserved() {
        let contents = r#"
[params]
rust_version = "1.75"
some_other = "value"
"#;

        let config = parse_config(contents).unwrap();
        assert_eq!(config.params.get("rust_version"), Some(&"1.75".to_string()));
        assert_eq!(config.params.get("some_other"), Some(&"value".to_string()));

        let merged = ConfigMerger::new(config).merge_plan_args(&[], &[], false, &HashMap::new());

        assert_eq!(merged.params.get("rust_version"), Some(&"1.75".to_string()));
    }

    #[test]
    fn test_parse_cli_params_valid() {
        let params = vec!["key=value".to_string(), "other=two".to_string()];
        let parsed = parse_cli_params(&params).expect("parse params");
        assert_eq!(parsed.get("key"), Some(&"value".to_string()));
        assert_eq!(parsed.get("other"), Some(&"two".to_string()));
    }

    #[test]
    fn test_parse_cli_params_missing_key() {
        let params = vec!["=value".to_string()];
        let err = parse_cli_params(&params).expect_err("missing key");
        assert!(err.to_string().contains("missing key"));
    }

    #[test]
    fn test_parse_cli_params_missing_value() {
        let params = vec!["key=".to_string()];
        let err = parse_cli_params(&params).expect_err("missing value");
        assert!(err.to_string().contains("missing value"));
    }

    #[test]
    fn test_discover_config_some_and_none() {
        let temp = TempDir::new().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
        assert!(discover_config(&root).is_none());

        std::fs::write(root.join(CONFIG_FILE_NAME), "").expect("write config");
        assert!(discover_config(&root).is_some());
    }

    #[test]
    fn test_load_or_default_returns_default_when_missing() {
        let temp = TempDir::new().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
        let cfg = load_or_default(&root).expect("load default");
        assert!(cfg.policy.allow.is_empty());
        assert!(cfg.policy.deny.is_empty());
        assert!(cfg.backups.enabled);
    }
}
