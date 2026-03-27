//! Integration tests for buildfix-fixer-resolver-v2
//!
//! These tests complement the inline tests in src/resolver_v2.rs

use buildfix_fixer_api::{Fixer, PlanContext, PlannerConfig, ReceiptSet, RepoView};
use buildfix_fixer_resolver_v2::ResolverV2Fixer;
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::OpKind;
use buildfix_types::ops::SafetyClass;
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

/// Create a receipt set with a resolver v2 finding
fn receipt_set_with_resolver_finding(check_id: &str, code: &str) -> ReceiptSet {
    let receipt = ReceiptEnvelope {
        schema: "sensor.report.v1".to_string(),
        tool: ToolInfo {
            name: "cargo".to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict::default(),
        findings: vec![Finding {
            severity: Default::default(),
            check_id: Some(check_id.to_string()),
            code: Some(code.to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from("Cargo.toml"),
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
        path: Utf8PathBuf::from("artifacts/cargo/report.json"),
        sensor_id: "cargo".to_string(),
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
    let fixer = ResolverV2Fixer;
    let meta = fixer.meta();
    assert_eq!(meta.fix_key, "cargo.workspace_resolver_v2");
}

#[test]
fn fixer_meta_returns_correct_description() {
    let fixer = ResolverV2Fixer;
    let meta = fixer.meta();
    assert!(!meta.description.is_empty());
    assert!(meta.description.contains("resolver"));
}

#[test]
fn fixer_meta_returns_safe_safety_class() {
    let fixer = ResolverV2Fixer;
    let meta = fixer.meta();
    assert_eq!(meta.safety, SafetyClass::Safe);
}

#[test]
fn fixer_meta_declares_sensors() {
    let fixer = ResolverV2Fixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_sensors.is_empty());
}

#[test]
fn fixer_meta_declares_check_ids() {
    let fixer = ResolverV2Fixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_check_ids.is_empty());
}

// ============================================================================
// Plan Generation Tests
// ============================================================================

#[test]
fn plan_returns_empty_when_no_receipts() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace]
resolver = "1""#,
    )]);
    let ctx = plan_context();

    let ops = fixer.plan(&ctx, &repo, &empty_receipt_set()).unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_manifest_has_resolver_v2() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace]
resolver = "2""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_resolver_finding("workspace.resolver_v2", "RESOLVER"),
        )
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_not_a_workspace() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[package]
name = "demo""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_resolver_finding("workspace.resolver_v2", "RESOLVER"),
        )
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_generates_op_when_resolver_is_v1() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace]
resolver = "1""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_resolver_finding("workspace.resolver_v2", "RESOLVER"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Verify safety class
    assert_eq!(op.safety, SafetyClass::Safe);

    // Verify target
    assert_eq!(op.target.path, "Cargo.toml");

    // Verify operation kind
    assert!(matches!(
        &op.kind,
        OpKind::TomlTransform { rule_id, args } if rule_id == "ensure_workspace_resolver_v2" && args.is_none()
    ));
}

#[test]
fn plan_generates_op_when_resolver_is_missing() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace]
members = ["crates/*"]"#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_resolver_finding("workspace.resolver_v2", "RESOLVER"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_generates_op_with_correct_rationale() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace]
resolver = "1""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_resolver_finding("workspace.resolver_v2", "RESOLVER"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Verify rationale
    assert!(op.rationale.description.is_some());
    assert!(!op.rationale.findings.is_empty());
    assert!(op.rationale.fix_key.contains("cargo"));
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[test]
fn plan_handles_missing_manifest_gracefully() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::empty();
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_resolver_finding("workspace.resolver_v2", "RESOLVER"),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_handles_invalid_toml_gracefully() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[("Cargo.toml", "not valid toml [")]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_resolver_finding("workspace.resolver_v2", "RESOLVER"),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_handles_empty_manifest_gracefully() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[("Cargo.toml", "")]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_resolver_finding("workspace.resolver_v2", "RESOLVER"),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_ignores_unrelated_check_ids() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace]
resolver = "1""#,
    )]);
    let ctx = plan_context();

    // Create a receipt with an unrelated check ID
    let receipt = ReceiptEnvelope {
        schema: "sensor.report.v1".to_string(),
        tool: ToolInfo {
            name: "cargo".to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict::default(),
        findings: vec![Finding {
            severity: Default::default(),
            check_id: Some("unrelated.check".to_string()),
            code: Some("CODE".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from("Cargo.toml"),
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
        path: Utf8PathBuf::from("artifacts/cargo/report.json"),
        sensor_id: "cargo".to_string(),
        receipt: Ok(receipt),
    }];
    let receipt_set = ReceiptSet::from_loaded(&loaded);

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert!(ops.is_empty());
}

// ============================================================================
// Alternative Check IDs
// ============================================================================

#[test]
fn plan_responds_to_cargo_prefix_check_id() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace]
resolver = "1""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_resolver_finding("cargo.workspace.resolver_v2", "RESOLVER"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

// ============================================================================
// Multiple Findings Tests
// ============================================================================

#[test]
fn plan_handles_multiple_findings() {
    let fixer = ResolverV2Fixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace]
resolver = "1""#,
    )]);
    let ctx = plan_context();

    let receipt = ReceiptEnvelope {
        schema: "sensor.report.v1".to_string(),
        tool: ToolInfo {
            name: "builddiag".to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict::default(),
        findings: vec![
            Finding {
                severity: Default::default(),
                check_id: Some("workspace.resolver_v2".to_string()),
                code: Some("RESOLVER".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from("Cargo.toml"),
                    line: Some(1),
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
            },
            Finding {
                severity: Default::default(),
                check_id: Some("workspace.resolver_v2".to_string()),
                code: Some("RESOLVER_V2".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from("Cargo.toml"),
                    line: Some(5),
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
            },
        ],
        capabilities: None,
        data: None,
    };

    let loaded = vec![LoadedReceipt {
        path: Utf8PathBuf::from("artifacts/builddiag/report.json"),
        sensor_id: "builddiag".to_string(),
        receipt: Ok(receipt),
    }];
    let receipt_set = ReceiptSet::from_loaded(&loaded);

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();

    // Should still produce a single operation
    assert_eq!(ops.len(), 1);

    // But rationale should include all findings
    assert_eq!(ops[0].rationale.findings.len(), 2);
}
