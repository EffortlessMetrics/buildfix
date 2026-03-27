//! Integration tests for buildfix-fixer-msrv
//!
//! These tests complement the inline tests in src/msrv.rs

use buildfix_fixer_api::{Fixer, PlanContext, PlannerConfig, ReceiptSet, RepoView};
use buildfix_fixer_msrv::MsrvNormalizeFixer;
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::{OpKind, SafetyClass};
use buildfix_types::receipt::{
    Finding, FindingContext, Location, ReceiptEnvelope, RunInfo, ToolInfo, Verdict,
    WorkspaceContext,
};
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

/// Create a receipt set with an MSRV finding
fn receipt_set_with_msrv_finding(sensor: &str, check_id: &str, path: &str) -> ReceiptSet {
    let receipt = ReceiptEnvelope {
        schema: "sensor.report.v1".to_string(),
        tool: ToolInfo {
            name: sensor.to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict::default(),
        findings: vec![Finding {
            severity: Default::default(),
            check_id: Some(check_id.to_string()),
            code: Some("MSRV".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from(path),
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
        path: Utf8PathBuf::from("artifacts/builddiag/report.json"),
        sensor_id: sensor.to_string(),
        receipt: Ok(receipt),
    }];
    ReceiptSet::from_loaded(&loaded)
}

/// Create a receipt set with evidence (confidence and context)
fn receipt_set_with_evidence(
    sensor: &str,
    check_id: &str,
    path: &str,
    confidence: Option<f64>,
    context: Option<FindingContext>,
) -> ReceiptSet {
    let receipt = ReceiptEnvelope {
        schema: "sensor.report.v1".to_string(),
        tool: ToolInfo {
            name: sensor.to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict::default(),
        findings: vec![Finding {
            severity: Default::default(),
            check_id: Some(check_id.to_string()),
            code: Some("MSRV".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from(path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: None,
            confidence,
            provenance: None,
            context,
        }],
        capabilities: None,
        data: None,
    };

    let loaded = vec![LoadedReceipt {
        path: Utf8PathBuf::from("artifacts/builddiag/report.json"),
        sensor_id: sensor.to_string(),
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
    let fixer = MsrvNormalizeFixer;
    let meta = fixer.meta();
    assert_eq!(meta.fix_key, "cargo.normalize_rust_version");
}

#[test]
fn fixer_meta_returns_correct_description() {
    let fixer = MsrvNormalizeFixer;
    let meta = fixer.meta();
    assert!(!meta.description.is_empty());
    assert!(meta.description.contains("rust-version") || meta.description.contains("MSRV"));
}

#[test]
fn fixer_meta_returns_guarded_safety_class() {
    let fixer = MsrvNormalizeFixer;
    let meta = fixer.meta();
    assert_eq!(meta.safety, SafetyClass::Guarded);
}

#[test]
fn fixer_meta_declares_sensors() {
    let fixer = MsrvNormalizeFixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_sensors.is_empty());
}

#[test]
fn fixer_meta_declares_check_ids() {
    let fixer = MsrvNormalizeFixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_check_ids.is_empty());
}

// ============================================================================
// Plan Generation Tests
// ============================================================================

#[test]
fn plan_returns_empty_when_no_receipts() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace.package]
rust-version = "1.70""#,
    )]);
    let ctx = plan_context();

    let ops = fixer.plan(&ctx, &repo, &empty_receipt_set()).unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_rust_version_already_matches() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.70""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_generates_guarded_op_when_rust_version_mismatch() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.60""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Default safety is Guarded (without full evidence)
    assert_eq!(op.safety, SafetyClass::Guarded);

    // Verify target
    assert_eq!(op.target.path, "crates/a/Cargo.toml");

    // Verify operation kind
    match &op.kind {
        OpKind::TomlTransform { rule_id, args } => {
            assert_eq!(rule_id, "set_package_rust_version");
            assert_eq!(args.as_ref().unwrap()["rust_version"], "1.70");
        }
        _ => panic!("expected TomlTransform operation"),
    }
}

#[test]
fn plan_generates_unsafe_op_without_workspace_canonical() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"
rust-version = "1.60""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Without workspace canonical, operation is Unsafe
    assert_eq!(op.safety, SafetyClass::Unsafe);
    assert_eq!(op.params_required, vec!["rust_version"]);
}

#[test]
fn plan_generates_op_with_correct_rationale() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.60""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Verify rationale
    assert!(op.rationale.description.is_some());
    assert!(!op.rationale.findings.is_empty());
    assert!(op.rationale.fix_key.contains("builddiag"));
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[test]
fn plan_handles_missing_manifest_gracefully() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::empty();
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_handles_invalid_toml_gracefully() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[("crates/a/Cargo.toml", "not valid toml [")]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();

    // Should still generate an op (needs_change returns true for invalid TOML)
    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_ignores_unrelated_check_ids() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.60""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "unrelated.check", "crates/a/Cargo.toml"),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_ignores_non_manifest_paths() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        ("crates/a/src/lib.rs", "// not a manifest"),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/src/lib.rs"),
        )
        .unwrap();

    assert!(ops.is_empty());
}

// ============================================================================
// Safety Promotion Tests
// ============================================================================

#[test]
fn plan_promotes_to_safe_with_full_evidence() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.60""#,
        ),
    ]);
    let ctx = plan_context();

    // High confidence + full consensus → Safe
    let receipt_set = receipt_set_with_evidence(
        "builddiag",
        "msrv.consistent",
        "crates/a/Cargo.toml",
        Some(0.95),
        Some(FindingContext {
            workspace: Some(WorkspaceContext {
                all_crates_agree: true,
                ..Default::default()
            }),
            analysis_depth: None,
        }),
    );

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].safety, SafetyClass::Safe);
}

#[test]
fn plan_remains_guarded_with_low_confidence() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.60""#,
        ),
    ]);
    let ctx = plan_context();

    // Low confidence → Guarded (not promoted to Safe)
    let receipt_set = receipt_set_with_evidence(
        "builddiag",
        "msrv.consistent",
        "crates/a/Cargo.toml",
        Some(0.7), // Below 0.9 threshold
        Some(FindingContext {
            workspace: Some(WorkspaceContext {
                all_crates_agree: true,
                ..Default::default()
            }),
            analysis_depth: None,
        }),
    );

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].safety, SafetyClass::Guarded);
}

#[test]
fn plan_remains_guarded_without_consensus() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.60""#,
        ),
    ]);
    let ctx = plan_context();

    // No consensus → Guarded
    let receipt_set = receipt_set_with_evidence(
        "builddiag",
        "msrv.consistent",
        "crates/a/Cargo.toml",
        Some(0.95),
        Some(FindingContext {
            workspace: Some(WorkspaceContext {
                all_crates_agree: false,
                ..Default::default()
            }),
            analysis_depth: None,
        }),
    );

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].safety, SafetyClass::Guarded);
}

// ============================================================================
// Multiple Check ID Support Tests
// ============================================================================

#[test]
fn plan_responds_to_rust_msrv_consistent_check_id() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.60""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding(
                "builddiag",
                "rust.msrv_consistent",
                "crates/a/Cargo.toml",
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_responds_to_cargo_msrv_consistent_check_id() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.60""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("cargo", "cargo.msrv_consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

// ============================================================================
// Workspace Canonical Rust-Version Resolution Tests
// ============================================================================

#[test]
fn plan_uses_workspace_package_rust_version_as_canonical() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.75"

[package]
rust-version = "1.60""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.50""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    match &ops[0].kind {
        OpKind::TomlTransform { args, .. } => {
            // Should use workspace.package.rust-version (1.75), not package.rust-version (1.60)
            assert_eq!(args.as_ref().unwrap()["rust_version"], "1.75");
        }
        _ => panic!("expected TomlTransform"),
    }
}

#[test]
fn plan_falls_back_to_package_rust_version_when_no_workspace_package() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
rust-version = "1.60""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    match &ops[0].kind {
        OpKind::TomlTransform { args, .. } => {
            // Should fall back to package.rust-version (1.70)
            assert_eq!(args.as_ref().unwrap()["rust_version"], "1.70");
        }
        _ => panic!("expected TomlTransform"),
    }
}

// ============================================================================
// Missing Rust-Version Field Tests
// ============================================================================

#[test]
fn plan_generates_op_when_crate_missing_rust_version() {
    let fixer = MsrvNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
rust-version = "1.70""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_msrv_finding("builddiag", "msrv.consistent", "crates/a/Cargo.toml"),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];
    assert_eq!(op.safety, SafetyClass::Guarded);
    assert_eq!(op.target.path, "crates/a/Cargo.toml");
}
