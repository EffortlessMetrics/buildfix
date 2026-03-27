//! Integration tests for buildfix-fixer-path-dep-version
//!
//! These tests complement the inline tests in src/path_dep_version.rs

use buildfix_fixer_api::{Fixer, PlanContext, PlannerConfig, ReceiptSet, RepoView};
use buildfix_fixer_path_dep_version::PathDepVersionFixer;
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::{OpKind, SafetyClass};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, RunInfo, ToolInfo, Verdict};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;

/// Mock repository for testing
struct MockRepo {
    root: Utf8PathBuf,
    files: HashMap<String, String>,
}

impl MockRepo {
    fn new(files: &[(&str, &str)]) -> Self {
        let mut map = HashMap::new();
        for (path, contents) in files {
            map.insert(path.to_string(), contents.to_string());
        }
        Self {
            root: Utf8PathBuf::from("."),
            files: map,
        }
    }

    fn empty() -> Self {
        Self {
            root: Utf8PathBuf::from("."),
            files: HashMap::new(),
        }
    }
}

impl RepoView for MockRepo {
    fn root(&self) -> &Utf8Path {
        &self.root
    }

    fn read_to_string(&self, rel: &Utf8Path) -> anyhow::Result<String> {
        let key = if rel.is_absolute() {
            rel.strip_prefix(&self.root).unwrap_or(rel).to_string()
        } else {
            rel.to_string()
        }
        .replace('\\', "/");
        self.files
            .get(&key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing {}", key))
    }

    fn exists(&self, rel: &Utf8Path) -> bool {
        let key = if rel.is_absolute() {
            rel.strip_prefix(&self.root).unwrap_or(rel).to_string()
        } else {
            rel.to_string()
        }
        .replace('\\', "/");
        self.files.contains_key(&key)
    }
}

/// Create a receipt set with a path dep version finding
fn receipt_set_with_finding(manifest_path: &str) -> ReceiptSet {
    let receipt = ReceiptEnvelope {
        schema: "sensor.report.v1".to_string(),
        tool: ToolInfo {
            name: "depguard".to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict::default(),
        findings: vec![Finding {
            severity: Default::default(),
            check_id: Some("deps.path_requires_version".to_string()),
            code: Some("missing_version".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from(manifest_path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: None,
            ..Default::default()
        }],
        capabilities: None,
        data: None,
    };

    let loaded = vec![LoadedReceipt {
        path: Utf8PathBuf::from("artifacts/depguard/report.json"),
        sensor_id: "depguard".to_string(),
        receipt: Ok(receipt),
    }];
    ReceiptSet::from_loaded(&loaded)
}

/// Create a receipt set with multiple findings
fn receipt_set_with_multiple_findings(manifest_paths: &[&str]) -> ReceiptSet {
    let findings: Vec<Finding> = manifest_paths
        .iter()
        .map(|path| Finding {
            severity: Default::default(),
            check_id: Some("deps.path_requires_version".to_string()),
            code: Some("missing_version".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from(*path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: None,
            ..Default::default()
        })
        .collect();

    let receipt = ReceiptEnvelope {
        schema: "sensor.report.v1".to_string(),
        tool: ToolInfo {
            name: "depguard".to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict::default(),
        findings,
        capabilities: None,
        data: None,
    };

    let loaded = vec![LoadedReceipt {
        path: Utf8PathBuf::from("artifacts/depguard/report.json"),
        sensor_id: "depguard".to_string(),
        receipt: Ok(receipt),
    }];
    ReceiptSet::from_loaded(&loaded)
}

/// Create an empty receipt set (no findings)
fn empty_receipt_set() -> ReceiptSet {
    let loaded: Vec<LoadedReceipt> = vec![];
    ReceiptSet::from_loaded(&loaded)
}

fn plan_context() -> PlanContext {
    PlanContext {
        repo_root: Utf8PathBuf::from("."),
        artifacts_dir: Utf8PathBuf::from("artifacts"),
        config: PlannerConfig::default(),
    }
}

// ============================================================================
// Fixer Metadata Tests
// ============================================================================

#[test]
fn fixer_meta_returns_correct_fix_key() {
    let fixer = PathDepVersionFixer;
    let meta = fixer.meta();
    assert_eq!(meta.fix_key, "cargo.path_dep_add_version");
}

#[test]
fn fixer_meta_returns_correct_description() {
    let fixer = PathDepVersionFixer;
    let meta = fixer.meta();
    assert!(!meta.description.is_empty());
    assert!(meta.description.contains("version"));
}

#[test]
fn fixer_meta_returns_safe_safety_class() {
    let fixer = PathDepVersionFixer;
    let meta = fixer.meta();
    assert_eq!(meta.safety, SafetyClass::Safe);
}

#[test]
fn fixer_meta_declares_depguard_sensor() {
    let fixer = PathDepVersionFixer;
    let meta = fixer.meta();
    assert!(meta.consumes_sensors.contains(&"depguard"));
}

#[test]
fn fixer_meta_declares_check_ids() {
    let fixer = PathDepVersionFixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_check_ids.is_empty());
}

// ============================================================================
// Plan Generation Tests - Empty/Missing Data
// ============================================================================

#[test]
fn plan_returns_empty_when_no_receipts() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[(
        "crates/app/Cargo.toml",
        r#"[package]
name = "app"
[dependencies]
dep = { path = "../dep" }"#,
    )]);
    let ctx = plan_context();

    let ops = fixer.plan(&ctx, &repo, &empty_receipt_set()).unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_manifest_missing() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::empty();
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_manifest_invalid_toml() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[("crates/app/Cargo.toml", "not valid toml [")]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();
    assert!(ops.is_empty());
}

// ============================================================================
// Plan Generation Tests - With Version Inference
// ============================================================================

#[test]
fn plan_infers_version_from_target_crate() {
    let fixer = PathDepVersionFixer;
    // Use the same structure as the inline test: dep is in crates/app/dep/
    let repo = MockRepo::new(&[
        (
            "crates/app/Cargo.toml",
            r#"[package]
name = "app"
[dependencies]
dep = { path = "dep" }"#,
        ),
        (
            "crates/app/dep/Cargo.toml",
            r#"[package]
name = "dep"
version = "2.0.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Should be safe since version was inferred
    assert_eq!(op.safety, SafetyClass::Safe);

    // Verify args contain the version
    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        assert_eq!(args["version"], "2.0.0");
        assert_eq!(args["dep"], "dep");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_infers_version_from_workspace_package() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
version = "1.5.0""#,
        ),
        (
            "crates/app/Cargo.toml",
            r#"[package]
name = "app"
[dependencies]
dep = { path = "../dep" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Should be safe since version was inferred
    assert_eq!(op.safety, SafetyClass::Safe);

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        assert_eq!(args["version"], "1.5.0");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_marks_unsafe_when_version_unknown() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[(
        "crates/app/Cargo.toml",
        r#"[package]
name = "app"
[dependencies]
dep = { path = "../dep" }"#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Should be unsafe since version could not be inferred
    assert_eq!(op.safety, SafetyClass::Unsafe);
    assert_eq!(op.params_required, vec!["version"]);
}

// ============================================================================
// Plan Generation Tests - Multiple Dependencies
// ============================================================================

#[test]
fn plan_handles_multiple_path_deps() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
version = "1.0.0""#,
        ),
        (
            "crates/app/Cargo.toml",
            r#"[package]
name = "app"
[dependencies]
dep1 = { path = "../dep1" }
dep2 = { path = "../dep2" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    // Should have ops for both path deps
    assert_eq!(ops.len(), 2);

    // All should be safe (version inferred from workspace)
    for op in &ops {
        assert_eq!(op.safety, SafetyClass::Safe);
    }
}

#[test]
fn plan_handles_dev_dependencies() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
version = "1.0.0""#,
        ),
        (
            "crates/app/Cargo.toml",
            r#"[package]
name = "app"
[dev-dependencies]
test-dep = { path = "../test-dep" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        assert_eq!(args["dep"], "test-dep");
        // Verify toml_path starts with dev-dependencies
        let toml_path = args["toml_path"].as_array().expect("toml_path array");
        assert_eq!(toml_path[0], "dev-dependencies");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_handles_build_dependencies() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
version = "1.0.0""#,
        ),
        (
            "crates/app/Cargo.toml",
            r#"[package]
name = "app"
[build-dependencies]
build-dep = { path = "../build-dep" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        assert_eq!(args["dep"], "build-dep");
        let toml_path = args["toml_path"].as_array().expect("toml_path array");
        assert_eq!(toml_path[0], "build-dependencies");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_handles_target_specific_dependencies() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
version = "1.0.0""#,
        ),
        (
            "crates/app/Cargo.toml",
            r#"[package]
name = "app"
[target.'cfg(windows)'.dependencies]
win-dep = { path = "../win-dep" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        assert_eq!(args["dep"], "win-dep");
        let toml_path = args["toml_path"].as_array().expect("toml_path array");
        assert_eq!(toml_path[0], "target");
        assert_eq!(toml_path[1], "cfg(windows)");
        assert_eq!(toml_path[2], "dependencies");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

// ============================================================================
// Plan Generation Tests - Skips Non-Applicable Deps
// ============================================================================

#[test]
fn plan_skips_deps_with_version_already() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[(
        "crates/app/Cargo.toml",
        r#"[package]
name = "app"
[dependencies]
dep = { path = "../dep", version = "1.0.0" }"#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    // Should not produce ops for deps that already have version
    assert!(ops.is_empty());
}

#[test]
fn plan_skips_workspace_true_deps() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[(
        "crates/app/Cargo.toml",
        r#"[package]
name = "app"
[dependencies]
dep = { path = "../dep", workspace = true }"#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_skips_non_path_deps() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[(
        "crates/app/Cargo.toml",
        r#"[package]
name = "app"
[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert!(ops.is_empty());
}

// ============================================================================
// Plan Generation Tests - Multiple Manifests
// ============================================================================

#[test]
fn plan_handles_multiple_manifests() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
version = "1.0.0""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[dependencies]
dep = { path = "../dep" }"#,
        ),
        (
            "crates/b/Cargo.toml",
            r#"[package]
name = "b"
[dependencies]
dep = { path = "../dep" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_multiple_findings(&["crates/a/Cargo.toml", "crates/b/Cargo.toml"]),
        )
        .unwrap();

    // Should have ops for both manifests
    assert_eq!(ops.len(), 2);

    // Each should target different manifests
    let paths: std::collections::HashSet<_> =
        ops.iter().map(|op| op.target.path.as_str()).collect();
    assert!(paths.contains("crates/a/Cargo.toml"));
    assert!(paths.contains("crates/b/Cargo.toml"));
}

// ============================================================================
// Rationale Tests
// ============================================================================

#[test]
fn plan_includes_rationale_with_findings() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
version = "1.0.0""#,
        ),
        (
            "crates/app/Cargo.toml",
            r#"[package]
name = "app"
[dependencies]
dep = { path = "../dep" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    assert!(op.rationale.description.is_some());
    assert!(!op.rationale.findings.is_empty());
    assert!(op.rationale.fix_key.contains("depguard"));
}

// ============================================================================
// Table Style Dependencies
// ============================================================================

#[test]
fn plan_handles_table_style_dependencies() {
    let fixer = PathDepVersionFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
version = "1.0.0""#,
        ),
        (
            "crates/app/Cargo.toml",
            r#"[package]
name = "app"
[dependencies.dep]
path = "../dep""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/app/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        assert_eq!(args["dep"], "dep");
    } else {
        panic!("Expected TomlTransform with args");
    }
}
