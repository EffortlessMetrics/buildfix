//! Integration tests for buildfix-fixer-duplicate-deps
//!
//! These tests complement the inline tests in src/duplicate_deps.rs

use buildfix_fixer_api::{Fixer, PlanContext, PlannerConfig, ReceiptSet, RepoView};
use buildfix_fixer_duplicate_deps::DuplicateDepsConsolidationFixer;
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
        };
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
        };
        self.files.contains_key(&key)
    }
}

/// Create a duplicate dependency finding
fn duplicate_finding(
    manifest_path: &str,
    dep: &str,
    selected_version: &str,
    toml_path: &[&str],
) -> Finding {
    Finding {
        severity: Default::default(),
        check_id: Some("deps.duplicate_dependency_versions".to_string()),
        code: Some("duplicate_version".to_string()),
        message: Some("duplicate dependency versions".to_string()),
        location: Some(Location {
            path: Utf8PathBuf::from(manifest_path),
            line: Some(1),
            column: None,
        }),
        fingerprint: None,
        data: Some(serde_json::json!({
            "dep": dep,
            "selected_version": selected_version,
            "toml_path": toml_path,
        })),
        ..Default::default()
    }
}

/// Create a receipt set with duplicate dependency findings
fn receipt_set_with_findings(findings: Vec<Finding>) -> ReceiptSet {
    let receipt = ReceiptEnvelope {
        schema: "depguard.report.v1".to_string(),
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
    let fixer = DuplicateDepsConsolidationFixer;
    let meta = fixer.meta();
    assert_eq!(meta.fix_key, "cargo.consolidate_duplicate_deps");
}

#[test]
fn fixer_meta_returns_correct_description() {
    let fixer = DuplicateDepsConsolidationFixer;
    let meta = fixer.meta();
    assert!(!meta.description.is_empty());
    assert!(meta.description.contains("duplicate"));
}

#[test]
fn fixer_meta_returns_safe_safety_class() {
    let fixer = DuplicateDepsConsolidationFixer;
    let meta = fixer.meta();
    assert_eq!(meta.safety, SafetyClass::Safe);
}

#[test]
fn fixer_meta_declares_depguard_sensor() {
    let fixer = DuplicateDepsConsolidationFixer;
    let meta = fixer.meta();
    assert!(meta.consumes_sensors.contains(&"depguard"));
}

#[test]
fn fixer_meta_declares_check_ids() {
    let fixer = DuplicateDepsConsolidationFixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_check_ids.is_empty());
}

// ============================================================================
// Plan Generation Tests - Empty/Missing Data
// ============================================================================

#[test]
fn plan_returns_empty_when_no_receipts() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"
[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer.plan(&ctx, &repo, &empty_receipt_set()).unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_manifest_missing() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::empty();
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "serde",
        "1.0.200",
        &["dependencies", "serde"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_manifest_invalid_toml() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[("crates/a/Cargo.toml", "not valid toml [")]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "serde",
        "1.0.200",
        &["dependencies", "serde"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();
    assert!(ops.is_empty());
}

// ============================================================================
// Plan Generation Tests - Basic Consolidation
// ============================================================================

#[test]
fn plan_generates_workspace_and_member_ops() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[dependencies]
serde = "1.0.200""#,
        ),
        (
            "crates/b/Cargo.toml",
            r#"[package]
name = "b"
[dependencies]
serde = { version = "1.0.180", features = ["derive"] }"#,
        ),
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n",
        ),
    ]);
    let ctx = plan_context();

    let findings = vec![
        duplicate_finding(
            "crates/a/Cargo.toml",
            "serde",
            "1.0.200",
            &["dependencies", "serde"],
        ),
        duplicate_finding(
            "crates/b/Cargo.toml",
            "serde",
            "1.0.200",
            &["dependencies", "serde"],
        ),
    ];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();

    // Should have 3 ops: 2 member ops + 1 workspace op
    assert_eq!(ops.len(), 3);

    // Verify workspace op exists
    assert!(ops.iter().any(|op| {
        op.target.path == "Cargo.toml"
            && matches!(
                op.kind,
                OpKind::TomlTransform {
                    ref rule_id,
                    args: Some(_)
                } if rule_id == "ensure_workspace_dependency_version"
            )
    }));

    // Verify member ops exist
    let member_ops: Vec<&buildfix_types::plan::PlanOp> = ops
        .iter()
        .filter(|op| {
            matches!(
                op.kind,
                OpKind::TomlTransform {
                    ref rule_id,
                    args: Some(_)
                } if rule_id == "use_workspace_dependency"
            )
        })
        .collect();
    assert_eq!(member_ops.len(), 2);
}

#[test]
fn plan_preserves_features_in_member_op() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[dependencies]
serde = { version = "1.0.180", features = ["derive"] }"#,
        ),
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
    ]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "serde",
        "1.0.200",
        &["dependencies", "serde"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();

    // Find the member op
    let member_op = ops
        .iter()
        .find(|op| op.target.path == "crates/a/Cargo.toml")
        .expect("member op");

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &member_op.kind
    {
        let preserved = &args["preserved"];
        assert_eq!(preserved["features"], serde_json::json!(["derive"]));
    } else {
        panic!("Expected TomlTransform with args");
    }
}

// ============================================================================
// Plan Generation Tests - Version Conflicts
// ============================================================================

#[test]
fn plan_skips_when_selected_versions_conflict() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            "[package]\nname = \"a\"\n[dependencies]\nserde = \"1.0.0\"\n",
        ),
        (
            "crates/b/Cargo.toml",
            "[package]\nname = \"b\"\n[dependencies]\nserde = \"1.1.0\"\n",
        ),
    ]);
    let ctx = plan_context();

    let findings = vec![
        duplicate_finding(
            "crates/a/Cargo.toml",
            "serde",
            "1.1.0",
            &["dependencies", "serde"],
        ),
        duplicate_finding(
            "crates/b/Cargo.toml",
            "serde",
            "1.0.0",
            &["dependencies", "serde"],
        ),
    ];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();
    assert!(ops.is_empty());
}

// ============================================================================
// Plan Generation Tests - Missing Required Data
// ============================================================================

#[test]
fn plan_skips_when_toml_path_missing() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        "[package]\nname = \"a\"\n[dependencies]\nserde = \"1.0.0\"\n",
    )]);
    let ctx = plan_context();

    let finding = Finding {
        severity: Default::default(),
        check_id: Some("deps.duplicate_dependency_versions".to_string()),
        code: Some("duplicate_version".to_string()),
        message: None,
        location: Some(Location {
            path: Utf8PathBuf::from("crates/a/Cargo.toml"),
            line: Some(1),
            column: None,
        }),
        fingerprint: None,
        data: Some(serde_json::json!({
            "dep": "serde",
            "selected_version": "1.0.0"
            // Missing toml_path
        })),
        ..Default::default()
    };

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(vec![finding]))
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_skips_when_dep_name_missing() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        "[package]\nname = \"a\"\n[dependencies]\nserde = \"1.0.0\"\n",
    )]);
    let ctx = plan_context();

    let finding = Finding {
        severity: Default::default(),
        check_id: Some("deps.duplicate_dependency_versions".to_string()),
        code: Some("duplicate_version".to_string()),
        message: None,
        location: Some(Location {
            path: Utf8PathBuf::from("crates/a/Cargo.toml"),
            line: Some(1),
            column: None,
        }),
        fingerprint: None,
        data: Some(serde_json::json!({
            // Missing dep
            "selected_version": "1.0.0",
            "toml_path": ["dependencies", "serde"]
        })),
        ..Default::default()
    };

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(vec![finding]))
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_skips_when_selected_version_missing() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        "[package]\nname = \"a\"\n[dependencies]\nserde = \"1.0.0\"\n",
    )]);
    let ctx = plan_context();

    let finding = Finding {
        severity: Default::default(),
        check_id: Some("deps.duplicate_dependency_versions".to_string()),
        code: Some("duplicate_version".to_string()),
        message: None,
        location: Some(Location {
            path: Utf8PathBuf::from("crates/a/Cargo.toml"),
            line: Some(1),
            column: None,
        }),
        fingerprint: None,
        data: Some(serde_json::json!({
            "dep": "serde",
            // Missing selected_version
            "toml_path": ["dependencies", "serde"]
        })),
        ..Default::default()
    };

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(vec![finding]))
        .unwrap();
    assert!(ops.is_empty());
}

// ============================================================================
// Plan Generation Tests - Different Dependency Sections
// ============================================================================

#[test]
fn plan_handles_dev_dependencies() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[dev-dependencies]
serde = "1.0.200""#,
        ),
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
    ]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "serde",
        "1.0.200",
        &["dev-dependencies", "serde"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();

    // Should have 2 ops: 1 member op + 1 workspace op
    assert_eq!(ops.len(), 2);

    let member_op = ops
        .iter()
        .find(|op| op.target.path == "crates/a/Cargo.toml")
        .expect("member op");

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &member_op.kind
    {
        let toml_path = args["toml_path"].as_array().expect("toml_path array");
        assert_eq!(toml_path[0], "dev-dependencies");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_handles_build_dependencies() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[build-dependencies]
serde = "1.0.200""#,
        ),
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
    ]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "serde",
        "1.0.200",
        &["build-dependencies", "serde"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();

    assert_eq!(ops.len(), 2);

    let member_op = ops
        .iter()
        .find(|op| op.target.path == "crates/a/Cargo.toml")
        .expect("member op");

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &member_op.kind
    {
        let toml_path = args["toml_path"].as_array().expect("toml_path array");
        assert_eq!(toml_path[0], "build-dependencies");
    } else {
        panic!("Expected TomlTransform with args");
    }
}

#[test]
fn plan_handles_target_specific_dependencies() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[target.'cfg(windows)'.dependencies]
serde = "1.0.200""#,
        ),
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
    ]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "serde",
        "1.0.200",
        &["target", "cfg(windows)", "dependencies", "serde"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();

    assert_eq!(ops.len(), 2);

    let member_op = ops
        .iter()
        .find(|op| op.target.path == "crates/a/Cargo.toml")
        .expect("member op");

    if let OpKind::TomlTransform {
        args: Some(args), ..
    } = &member_op.kind
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
// Plan Generation Tests - Skip Non-Applicable Deps
// ============================================================================

#[test]
fn plan_skips_path_dependencies() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"
[dependencies]
local = { path = "../local", version = "0.1.0" }"#,
    )]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "local",
        "0.1.0",
        &["dependencies", "local"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();
    // Path deps should be skipped (dep_preserve_from_item returns None)
    assert!(ops.is_empty());
}

#[test]
fn plan_skips_git_dependencies() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"
[dependencies]
my-crate = { git = "https://github.com/example/crate", version = "1.0.0" }"#,
    )]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "my-crate",
        "1.0.0",
        &["dependencies", "my-crate"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();
    // Git deps should be skipped
    assert!(ops.is_empty());
}

#[test]
fn plan_skips_workspace_true_dependencies() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"
[dependencies]
serde = { workspace = true }"#,
    )]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "serde",
        "1.0.0",
        &["dependencies", "serde"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();
    // workspace = true deps should be skipped
    assert!(ops.is_empty());
}

// ============================================================================
// Safety Classification Tests
// ============================================================================

#[test]
fn plan_ops_are_safe() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[dependencies]
serde = "1.0.200""#,
        ),
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
    ]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "serde",
        "1.0.200",
        &["dependencies", "serde"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();

    // All ops should be Safe
    for op in &ops {
        assert_eq!(op.safety, SafetyClass::Safe);
    }
}

// ============================================================================
// Rationale Tests
// ============================================================================

#[test]
fn plan_includes_rationale_with_findings() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[dependencies]
serde = "1.0.200""#,
        ),
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
    ]);
    let ctx = plan_context();

    let findings = vec![duplicate_finding(
        "crates/a/Cargo.toml",
        "serde",
        "1.0.200",
        &["dependencies", "serde"],
    )];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();

    assert!(!ops.is_empty());
    for op in &ops {
        assert!(op.rationale.description.is_some());
        assert!(!op.rationale.findings.is_empty());
        assert!(op.rationale.fix_key.contains("depguard"));
    }
}

// ============================================================================
// Alternative Check IDs Tests
// ============================================================================

#[test]
fn plan_responds_to_duplicate_versions_check_id() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[dependencies]
serde = "1.0.200""#,
        ),
        ("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
    ]);
    let ctx = plan_context();

    let finding = Finding {
        severity: Default::default(),
        check_id: Some("deps.duplicate_versions".to_string()),
        code: Some("duplicate_version".to_string()),
        message: None,
        location: Some(Location {
            path: Utf8PathBuf::from("crates/a/Cargo.toml"),
            line: Some(1),
            column: None,
        }),
        fingerprint: None,
        data: Some(serde_json::json!({
            "dep": "serde",
            "selected_version": "1.0.200",
            "toml_path": ["dependencies", "serde"],
        })),
        ..Default::default()
    };

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(vec![finding]))
        .unwrap();
    assert!(!ops.is_empty());
}

// ============================================================================
// Multiple Dependencies Tests
// ============================================================================

#[test]
fn plan_handles_multiple_different_deps() {
    let fixer = DuplicateDepsConsolidationFixer;
    let repo = MockRepo::new(&[
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
[dependencies]
serde = "1.0.200"
tokio = "1.0.0""#,
        ),
        (
            "crates/b/Cargo.toml",
            r#"[package]
name = "b"
[dependencies]
serde = "1.0.180"
tokio = "1.0.0""#,
        ),
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n",
        ),
    ]);
    let ctx = plan_context();

    let findings = vec![
        duplicate_finding(
            "crates/a/Cargo.toml",
            "serde",
            "1.0.200",
            &["dependencies", "serde"],
        ),
        duplicate_finding(
            "crates/b/Cargo.toml",
            "serde",
            "1.0.200",
            &["dependencies", "serde"],
        ),
        duplicate_finding(
            "crates/a/Cargo.toml",
            "tokio",
            "1.0.0",
            &["dependencies", "tokio"],
        ),
        duplicate_finding(
            "crates/b/Cargo.toml",
            "tokio",
            "1.0.0",
            &["dependencies", "tokio"],
        ),
    ];

    let ops = fixer
        .plan(&ctx, &repo, &receipt_set_with_findings(findings))
        .unwrap();

    // Should have ops for both deps
    // 2 member ops for serde + 2 member ops for tokio + 2 workspace ops = 6
    assert_eq!(ops.len(), 6);
}
