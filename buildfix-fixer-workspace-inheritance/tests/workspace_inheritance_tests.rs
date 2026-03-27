//! Integration tests for buildfix-fixer-workspace-inheritance
//!
//! These tests complement the inline tests in src/workspace_inheritance.rs

use buildfix_fixer_api::{Fixer, PlanContext, PlannerConfig, ReceiptSet, RepoView};
use buildfix_fixer_workspace_inheritance::WorkspaceInheritanceFixer;
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

    #[allow(dead_code)]
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

/// Create a receipt set with a workspace inheritance finding
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
            check_id: Some("deps.workspace_inheritance".to_string()),
            code: Some("can_use_workspace".to_string()),
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
    let fixer = WorkspaceInheritanceFixer;
    let meta = fixer.meta();
    assert_eq!(meta.fix_key, "cargo.use_workspace_dependency");
}

#[test]
fn fixer_meta_returns_correct_description() {
    let fixer = WorkspaceInheritanceFixer;
    let meta = fixer.meta();
    assert!(!meta.description.is_empty());
    assert!(meta.description.contains("workspace"));
}

#[test]
fn fixer_meta_returns_safe_safety_class() {
    let fixer = WorkspaceInheritanceFixer;
    let meta = fixer.meta();
    assert_eq!(meta.safety, SafetyClass::Safe);
}

#[test]
fn fixer_meta_declares_depguard_sensor() {
    let fixer = WorkspaceInheritanceFixer;
    let meta = fixer.meta();
    assert!(meta.consumes_sensors.contains(&"depguard"));
}

#[test]
fn fixer_meta_declares_check_ids() {
    let fixer = WorkspaceInheritanceFixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_check_ids.is_empty());
}

// ============================================================================
// Plan Generation Tests - Empty/Missing Data
// ============================================================================

#[test]
fn plan_returns_empty_when_no_receipts() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer.plan(&ctx, &repo, &empty_receipt_set()).unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_no_workspace_deps() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace]
members = ["crates/member"]"#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_manifest_missing() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace.dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_manifest_invalid_toml() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        ("crates/member/Cargo.toml", "not valid toml ["),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();
    assert!(ops.is_empty());
}

// ============================================================================
// Plan Generation Tests - Basic Conversion
// ============================================================================

#[test]
fn plan_converts_simple_version_dep_to_workspace() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Should be safe since versions match
    assert_eq!(op.safety, SafetyClass::Safe);
    assert_eq!(op.target.path, "crates/member/Cargo.toml");

    if let OpKind::TomlTransform {
        rule_id,
        args: Some(args),
    } = &op.kind
    {
        assert_eq!(rule_id, "use_workspace_dependency");
        assert_eq!(args["dep"], "serde");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_converts_inline_table_dep_to_workspace() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }"#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = { version = "1.0" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    assert_eq!(op.safety, SafetyClass::Safe);

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        assert_eq!(args["dep"], "serde");
        // Should preserve any member-specific fields
        let preserved = &args["preserved"];
        assert!(preserved.is_object());
    } else {
        panic!("Expected TomlTransform with args");
    }
}

// ============================================================================
// Plan Generation Tests - Safety Classification
// ============================================================================

#[test]
fn plan_marks_guarded_for_version_mismatch() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = "2.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Should be guarded since member uses different version
    assert_eq!(op.safety, SafetyClass::Guarded);
}

#[test]
fn plan_marks_guarded_for_path_dep_in_workspace() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
local = { path = "crates/local", version = "0.1.0" }"#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
local = "0.1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Should be guarded since workspace dep is a path dep
    assert_eq!(op.safety, SafetyClass::Guarded);
}

// ============================================================================
// Plan Generation Tests - Skip Non-Applicable Deps
// ============================================================================

#[test]
fn plan_skips_deps_not_in_workspace() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
tokio = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    // tokio is not in workspace.dependencies, so no ops
    assert!(ops.is_empty());
}

#[test]
fn plan_skips_already_workspace_deps() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = { workspace = true }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_skips_path_deps_not_in_workspace() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
local = { path = "../local" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    // local is a path dep and not in workspace.dependencies
    assert!(ops.is_empty());
}

#[test]
fn plan_skips_git_deps() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
my-crate = { git = "https://github.com/example/crate" }"#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
my-crate = { git = "https://github.com/example/crate" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    // Git deps should be skipped
    assert!(ops.is_empty());
}

// ============================================================================
// Plan Generation Tests - Dependency Sections
// ============================================================================

#[test]
fn plan_handles_dev_dependencies() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dev-dependencies]
serde = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        let toml_path = args["toml_path"].as_array().expect("toml_path array");
        assert_eq!(toml_path[0], "dev-dependencies");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_handles_build_dependencies() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[build-dependencies]
serde = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        let toml_path = args["toml_path"].as_array().expect("toml_path array");
        assert_eq!(toml_path[0], "build-dependencies");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_handles_target_specific_dependencies() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[target.'cfg(windows)'.dependencies]
serde = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        let toml_path = args["toml_path"].as_array().expect("toml_path array");
        assert_eq!(toml_path[0], "target");
        assert_eq!(toml_path[1], "cfg(windows)");
        assert_eq!(toml_path[2], "dependencies");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

// ============================================================================
// Plan Generation Tests - Field Preservation
// ============================================================================

#[test]
fn plan_preserves_features() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = { version = "1.0", features = ["derive", "rc"] }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        let preserved = &args["preserved"];
        let features = preserved["features"].as_array().expect("features array");
        assert_eq!(features.len(), 2);
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_preserves_optional() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = { version = "1.0", optional = true }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        let preserved = &args["preserved"];
        assert_eq!(preserved["optional"], true);
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_preserves_package() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde-json = { package = "serde_json", version = "1.0" }"#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde-json = { package = "serde_json", version = "1.0" }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        let preserved = &args["preserved"];
        assert_eq!(preserved["package"], "serde_json");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_preserves_default_features() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = { version = "1.0", default-features = false }"#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        let preserved = &args["preserved"];
        // Note: the key uses underscore, not hyphen
        assert_eq!(preserved["default_features"], false);
    } else {
        panic!("Expected TomlTransform with args");
    }
}

// ============================================================================
// Plan Generation Tests - Multiple Dependencies
// ============================================================================

#[test]
fn plan_handles_multiple_deps() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0"
tokio = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = "1.0"
tokio = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 2);
}

// ============================================================================
// Rationale Tests
// ============================================================================

#[test]
fn plan_includes_rationale_with_findings() {
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies]
serde = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
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
    let fixer = WorkspaceInheritanceFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.dependencies]
serde = "1.0""#,
        ),
        (
            "crates/member/Cargo.toml",
            r#"[package]
name = "member"
[dependencies.serde]
version = "1.0""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_finding("crates/member/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &op.kind
    {
        assert_eq!(args["dep"], "serde");
    } else {
        panic!("Expected TomlTransform with args");
    }
}
