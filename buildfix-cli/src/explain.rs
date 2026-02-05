//! Fix explanation module for the `buildfix explain` command.
//!
//! Provides detailed explanations of each fix including:
//! - What the fix does
//! - Safety rationale
//! - Remediation guidance
//! - Triggering sensor findings

use buildfix_types::ops::SafetyClass;

/// Information about a buildfix fix.
#[derive(Debug, Clone)]
pub struct FixExplanation {
    /// Short key for the fix (user-facing, e.g., "resolver-v2").
    pub key: &'static str,
    /// Internal fix ID (e.g., "cargo.workspace_resolver_v2").
    pub fix_id: &'static str,
    /// Human-readable title.
    pub title: &'static str,
    /// Safety classification.
    pub safety: SafetyClass,
    /// Detailed description of what the fix does.
    pub description: &'static str,
    /// Safety rationale explaining why this fix has its safety class.
    pub safety_rationale: &'static str,
    /// Remediation guidance for manual fixes or when the fix is blocked.
    pub remediation: &'static str,
    /// Sensor findings that trigger this fix.
    pub triggers: &'static [TriggerPattern],
}

/// Pattern for matching sensor findings.
#[derive(Debug, Clone)]
pub struct TriggerPattern {
    /// Sensor/tool name (e.g., "builddiag", "depguard").
    pub sensor: &'static str,
    /// Check ID pattern (e.g., "workspace.resolver_v2").
    pub check_id: &'static str,
    /// Optional code pattern (e.g., "missing_version").
    pub code: Option<&'static str>,
}

/// Registry of all available fix explanations.
pub static FIX_REGISTRY: &[FixExplanation] = &[
    // 1) Workspace resolver v2
    FixExplanation {
        key: "resolver-v2",
        fix_id: "cargo.workspace_resolver_v2",
        title: "Workspace Resolver V2",
        safety: SafetyClass::Safe,
        description: r#"Sets `[workspace].resolver = "2"` in the root Cargo.toml.

Cargo's resolver v2 is the modern feature resolver that provides correct feature
unification across the dependency graph. It prevents surprising behavior where
dev-dependencies can enable features in normal dependencies.

This fix ensures your workspace uses the v2 resolver, which is required for:
- Correct feature handling in workspaces
- Avoiding feature leakage between different dependency kinds
- Compatibility with modern Cargo practices"#,
        safety_rationale: r#"This fix is classified as SAFE because:
- It only modifies the resolver field in the workspace table
- The change is deterministic and predictable
- Resolver v2 is backwards compatible for most projects
- The edit is trivially reversible
- It does not affect dependency versions or features directly"#,
        remediation: r#"To manually apply this fix, add or update your root Cargo.toml:

    [workspace]
    resolver = "2"

If this fix is blocked because the file is not a workspace, you may need to
convert your project to a workspace first or use `package.resolver = "2"`
for single-crate projects."#,
        triggers: &[
            TriggerPattern {
                sensor: "builddiag",
                check_id: "workspace.resolver_v2",
                code: None,
            },
            TriggerPattern {
                sensor: "cargo",
                check_id: "cargo.workspace.resolver_v2",
                code: None,
            },
        ],
    },
    // 2) Path dependency requires version
    FixExplanation {
        key: "path-dep-version",
        fix_id: "cargo.path_dep_add_version",
        title: "Path Dependency Version",
        safety: SafetyClass::Safe,
        description: r#"Adds a `version` field to path dependencies that are missing one.

Path dependencies without a version field cannot be published to crates.io. This
fix automatically determines the correct version by reading the target crate's
Cargo.toml and adds the version field.

Example transformation:
    foo = { path = "../foo" }
becomes:
    foo = { path = "../foo", version = "1.0.0" }"#,
        safety_rationale: r#"This fix is classified as SAFE when the version can be determined because:
- The version is read directly from the target crate's Cargo.toml
- No guesswork or heuristics are involved
- The edit is additive (only adds a field, doesn't change existing ones)
- The change is required for publishing and is policy-compliant

If the version cannot be determined (target crate has no version, or multiple
possible targets exist), the fix is skipped to maintain safety."#,
        remediation: r#"To manually apply this fix:

1. Find the target crate's Cargo.toml
2. Read the `[package].version` field
3. Add `version = "<version>"` to the path dependency

Example:
    [dependencies]
    my-crate = { path = "../my-crate", version = "0.1.0" }

If you need a different version constraint (e.g., ">=0.1"), manually edit
the version field after buildfix applies the exact version."#,
        triggers: &[
            TriggerPattern {
                sensor: "depguard",
                check_id: "deps.path_requires_version",
                code: Some("missing_version"),
            },
            TriggerPattern {
                sensor: "depguard",
                check_id: "cargo.path_requires_version",
                code: Some("missing_version"),
            },
        ],
    },
    // 3) Workspace dependency inheritance
    FixExplanation {
        key: "workspace-inheritance",
        fix_id: "cargo.use_workspace_dependency",
        title: "Workspace Dependency Inheritance",
        safety: SafetyClass::Safe,
        description: r#"Converts member crate dependencies to use workspace inheritance.

When a dependency is defined in [workspace.dependencies], member crates should
use `{ workspace = true }` instead of specifying the version directly. This
ensures version consistency across the workspace.

Example transformation:
    serde = "1.0"
becomes:
    serde = { workspace = true }

The fix preserves important per-crate overrides like:
- `features` - additional features for this crate
- `optional` - whether the dependency is optional
- `default-features` - whether to include default features
- `package` - renamed dependencies"#,
        safety_rationale: r#"This fix is classified as SAFE because:
- It only applies when the dependency exists in [workspace.dependencies]
- The transformation is deterministic and preserves override keys
- It enforces the single-source-of-truth pattern for versions
- The edit is easily reversible

The fix is skipped for:
- Dependencies already using workspace = true
- Path or git dependencies (these have different semantics)
- Dependencies not defined in workspace.dependencies"#,
        remediation: r#"To manually apply this fix:

1. Ensure the dependency is defined in root Cargo.toml:
    [workspace.dependencies]
    serde = { version = "1.0", features = ["derive"] }

2. Update member Cargo.toml to inherit:
    [dependencies]
    serde = { workspace = true }

3. Add per-crate overrides if needed:
    serde = { workspace = true, features = ["rc"] }

Note: The workspace definition controls version and base features.
Member crates can add features but cannot change the version."#,
        triggers: &[
            TriggerPattern {
                sensor: "depguard",
                check_id: "deps.workspace_inheritance",
                code: None,
            },
            TriggerPattern {
                sensor: "depguard",
                check_id: "cargo.workspace_inheritance",
                code: None,
            },
        ],
    },
    // 4) MSRV normalization
    FixExplanation {
        key: "msrv",
        fix_id: "cargo.normalize_rust_version",
        title: "MSRV Normalization",
        safety: SafetyClass::Guarded,
        description: r#"Normalizes per-crate rust-version (MSRV) declarations to match the workspace
canonical value.

The Minimum Supported Rust Version should be consistent across a workspace to
avoid confusion and ensure all crates can build with the same toolchain. This
fix sets member crate `package.rust-version` to match the workspace standard.

The canonical rust-version is determined from (in order):
1. [workspace.package].rust-version in root Cargo.toml
2. [package].rust-version in root Cargo.toml"#,
        safety_rationale: r#"This fix is classified as GUARDED because:
- Changing MSRV can affect which Rust versions can compile the crate
- A lower MSRV might hide newer Rust features being used
- A higher MSRV might break builds for users on older toolchains

The fix requires --allow-guarded because:
- It changes a semantic version constraint
- The impact depends on your support policy
- Manual review is recommended before applying

The fix is skipped entirely (not even planned) when:
- No canonical workspace rust-version exists
- The crate already has the correct rust-version"#,
        remediation: r#"To manually apply this fix:

1. Decide on your workspace's canonical MSRV
2. Set it in root Cargo.toml:
    [workspace.package]
    rust-version = "1.70"

3. Update each member crate:
    [package]
    rust-version = "1.70"
    # Or use workspace inheritance:
    rust-version.workspace = true

Before changing MSRV, verify your code compiles with the target version:
    cargo +1.70 check --workspace

Consider using cargo-msrv to verify actual minimum version."#,
        triggers: &[
            TriggerPattern {
                sensor: "builddiag",
                check_id: "rust.msrv_consistent",
                code: None,
            },
            TriggerPattern {
                sensor: "cargo",
                check_id: "cargo.msrv_consistent",
                code: None,
            },
            TriggerPattern {
                sensor: "cargo",
                check_id: "msrv.consistent",
                code: None,
            },
        ],
    },
    // 5) Edition normalization
    FixExplanation {
        key: "edition",
        fix_id: "cargo.normalize_edition",
        title: "Edition Normalization",
        safety: SafetyClass::Guarded,
        description: r#"Normalizes per-crate Rust edition declarations to match the workspace
canonical value.

The Rust edition determines which language features are available and how certain
syntax is interpreted. Having consistent editions across a workspace ensures:
- Predictable behavior across all crates
- Easier upgrades when moving to a new edition
- Clear documentation of language version requirements

The canonical edition is determined from (in order):
1. [workspace.package].edition in root Cargo.toml
2. [package].edition in root Cargo.toml"#,
        safety_rationale: r#"This fix is classified as GUARDED because:
- Changing edition can affect code semantics and compilation
- A higher edition may require code changes (e.g., 2021 keyword changes)
- A lower edition might disable features your code depends on

The fix requires --allow-guarded because:
- Edition changes can break builds
- The impact depends on which language features are used
- Manual review is recommended before applying

The fix is skipped entirely (not even planned) when:
- No canonical workspace edition exists
- The crate already has the correct edition"#,
        remediation: r#"To manually apply this fix:

1. Decide on your workspace's canonical edition
2. Set it in root Cargo.toml:
    [workspace.package]
    edition = "2021"

3. Update each member crate:
    [package]
    edition = "2021"
    # Or use workspace inheritance:
    edition.workspace = true

Before changing edition, verify your code compiles:
    cargo +nightly fix --edition

Consider using cargo fix to automatically migrate code between editions."#,
        triggers: &[
            TriggerPattern {
                sensor: "builddiag",
                check_id: "rust.edition_consistent",
                code: None,
            },
            TriggerPattern {
                sensor: "cargo",
                check_id: "cargo.edition_consistent",
                code: None,
            },
            TriggerPattern {
                sensor: "cargo",
                check_id: "edition.consistent",
                code: None,
            },
        ],
    },
];

/// Look up a fix explanation by key or fix_id.
pub fn lookup_fix(query: &str) -> Option<&'static FixExplanation> {
    let query_lower = query.to_lowercase();
    let query_normalized = query_lower.replace('_', "-");

    FIX_REGISTRY.iter().find(|fix| {
        // Match by key (e.g., "resolver-v2")
        fix.key == query_normalized
            // Match by fix_id (e.g., "cargo.workspace_resolver_v2")
            || fix.fix_id.to_lowercase() == query_lower
            // Match by partial fix_id suffix (e.g., "workspace_resolver_v2")
            || fix.fix_id.to_lowercase().ends_with(&format!(".{}", query_lower))
            // Match with underscores converted to hyphens
            || fix.fix_id.to_lowercase().replace('_', "-") == query_normalized
    })
}

/// List all available fix keys.
pub fn list_fix_keys() -> Vec<&'static str> {
    FIX_REGISTRY.iter().map(|f| f.key).collect()
}

/// Derive policy keys (sensor/check_id/code) from triggers.
pub fn policy_keys(fix: &FixExplanation) -> Vec<String> {
    let mut keys = std::collections::BTreeSet::new();
    for trigger in fix.triggers {
        let code = trigger.code.unwrap_or("*");
        keys.insert(format!("{}/{}/{}", trigger.sensor, trigger.check_id, code));
    }
    keys.into_iter().collect()
}

/// Format a safety class for display.
pub fn format_safety_class(safety: SafetyClass) -> &'static str {
    match safety {
        SafetyClass::Safe => "Safe",
        SafetyClass::Guarded => "Guarded",
        SafetyClass::Unsafe => "Unsafe",
    }
}

/// Get a description of what a safety class means.
pub fn safety_class_meaning(safety: SafetyClass) -> &'static str {
    match safety {
        SafetyClass::Safe => {
            "SAFE fixes are fully determined from repo-local truth and have low impact.\n\
             They are applied automatically with `buildfix apply --apply`."
        }
        SafetyClass::Guarded => {
            "GUARDED fixes are deterministic but have higher impact.\n\
             They require explicit approval with `buildfix apply --apply --allow-guarded`."
        }
        SafetyClass::Unsafe => {
            "UNSAFE fixes are ambiguous without user-provided inputs.\n\
             They are plan-only by default and require `--allow-unsafe` to apply."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_by_key() {
        let fix = lookup_fix("resolver-v2").expect("should find resolver-v2");
        assert_eq!(fix.key, "resolver-v2");
    }

    #[test]
    fn test_lookup_by_fix_id() {
        let fix = lookup_fix("cargo.workspace_resolver_v2").expect("should find by fix_id");
        assert_eq!(fix.key, "resolver-v2");
    }

    #[test]
    fn test_lookup_case_insensitive() {
        let fix = lookup_fix("RESOLVER-V2").expect("should find case insensitive");
        assert_eq!(fix.key, "resolver-v2");
    }

    #[test]
    fn test_lookup_underscores() {
        let fix = lookup_fix("resolver_v2").expect("should find with underscores");
        assert_eq!(fix.key, "resolver-v2");
    }

    #[test]
    fn test_all_fixes_registered() {
        assert_eq!(FIX_REGISTRY.len(), 5);
        assert!(lookup_fix("resolver-v2").is_some());
        assert!(lookup_fix("path-dep-version").is_some());
        assert!(lookup_fix("workspace-inheritance").is_some());
        assert!(lookup_fix("msrv").is_some());
        assert!(lookup_fix("edition").is_some());
    }
}
