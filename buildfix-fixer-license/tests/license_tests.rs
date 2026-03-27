//! Integration tests for buildfix-fixer-license
//!
//! These tests complement the inline tests in src/license.rs

use buildfix_fixer_api::{Fixer, PlanContext, PlannerConfig, ReceiptSet, RepoView};
use buildfix_fixer_license::LicenseNormalizeFixer;
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
        let normalized = key.replace('\\', "/");
        self.files
            .get(&normalized)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing {}", normalized))
    }

    fn exists(&self, rel: &Utf8Path) -> bool {
        let key = if rel.is_absolute() {
            rel.strip_prefix(&self.root).unwrap_or(rel).to_string()
        } else {
            rel.to_string()
        };
        let normalized = key.replace('\\', "/");
        self.files.contains_key(&normalized)
    }
}

/// Create a receipt set with a license finding
fn receipt_set_with_license_finding(
    sensor: &str,
    check_id: &str,
    path: &str,
    manifest_path: Option<&str>,
) -> ReceiptSet {
    let data = manifest_path.map(|mp| {
        serde_json::json!({
            "manifest_path": mp
        })
    });

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
            code: Some("missing_license".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from(path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data,
            ..Default::default()
        }],
        capabilities: None,
        data: None,
    };

    let loaded = vec![LoadedReceipt {
        path: Utf8PathBuf::from("artifacts/cargo-deny/report.json"),
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
    manifest_path: Option<&str>,
    confidence: Option<f64>,
    context: Option<FindingContext>,
) -> ReceiptSet {
    let data = manifest_path.map(|mp| {
        serde_json::json!({
            "manifest_path": mp
        })
    });

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
            code: Some("missing_license".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from(path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data,
            confidence,
            provenance: None,
            context,
        }],
        capabilities: None,
        data: None,
    };

    let loaded = vec![LoadedReceipt {
        path: Utf8PathBuf::from("artifacts/cargo-deny/report.json"),
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
    let fixer = LicenseNormalizeFixer;
    let meta = fixer.meta();
    assert_eq!(meta.fix_key, "cargo.normalize_license");
}

#[test]
fn fixer_meta_returns_correct_description() {
    let fixer = LicenseNormalizeFixer;
    let meta = fixer.meta();
    assert!(!meta.description.is_empty());
    assert!(meta.description.contains("license"));
}

#[test]
fn fixer_meta_returns_guarded_safety_class() {
    let fixer = LicenseNormalizeFixer;
    let meta = fixer.meta();
    assert_eq!(meta.safety, SafetyClass::Guarded);
}

#[test]
fn fixer_meta_declares_sensors() {
    let fixer = LicenseNormalizeFixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_sensors.is_empty());
    // Should include cargo-deny and deny
    assert!(
        meta.consumes_sensors
            .iter()
            .any(|s| *s == "cargo-deny" || *s == "deny")
    );
}

#[test]
fn fixer_meta_declares_check_ids() {
    let fixer = LicenseNormalizeFixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_check_ids.is_empty());
}

// ============================================================================
// Plan Generation Tests
// ============================================================================

#[test]
fn plan_returns_empty_when_no_receipts() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[(
        "Cargo.toml",
        r#"[workspace.package]
license = "MIT""#,
    )]);
    let ctx = plan_context();

    let ops = fixer.plan(&ctx, &repo, &empty_receipt_set()).unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_returns_empty_when_license_already_matches() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
license = "MIT""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();
    assert!(ops.is_empty());
}

#[test]
fn plan_generates_guarded_op_when_license_mismatch() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "Apache-2.0""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a"
license = "MIT""#,
        ),
    ]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
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
            assert_eq!(rule_id, "set_package_license");
            assert_eq!(args.as_ref().unwrap()["license"], "Apache-2.0");
        }
        _ => panic!("expected TomlTransform operation"),
    }
}

#[test]
fn plan_generates_unsafe_op_without_workspace_canonical() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Without workspace canonical, operation is Unsafe
    assert_eq!(op.safety, SafetyClass::Unsafe);
    assert_eq!(op.params_required, vec!["license"]);
}

#[test]
fn plan_generates_op_with_correct_rationale() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
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
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Verify rationale
    assert!(op.rationale.description.is_some());
    assert!(!op.rationale.findings.is_empty());
    assert!(op.rationale.fix_key.contains("cargo-deny"));
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[test]
fn plan_handles_missing_manifest_gracefully() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::empty();
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_ignores_unrelated_check_ids() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
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
            &receipt_set_with_license_finding(
                "cargo-deny",
                "unrelated.check",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert!(ops.is_empty());
}

// ============================================================================
// Safety Promotion Tests
// ============================================================================

#[test]
fn plan_promotes_to_safe_with_full_evidence() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a""#,
        ),
    ]);
    let ctx = plan_context();

    // High confidence + full consensus → Safe
    let receipt_set = receipt_set_with_evidence(
        "cargo-deny",
        "licenses.unlicensed",
        "crates/a/Cargo.toml",
        Some("crates/a/Cargo.toml"),
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
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a""#,
        ),
    ]);
    let ctx = plan_context();

    // Low confidence → Guarded (not promoted to Safe)
    let receipt_set = receipt_set_with_evidence(
        "cargo-deny",
        "licenses.unlicensed",
        "crates/a/Cargo.toml",
        Some("crates/a/Cargo.toml"),
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
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
        ),
        (
            "crates/a/Cargo.toml",
            r#"[package]
name = "a""#,
        ),
    ]);
    let ctx = plan_context();

    // No consensus → Guarded
    let receipt_set = receipt_set_with_evidence(
        "cargo-deny",
        "licenses.unlicensed",
        "crates/a/Cargo.toml",
        Some("crates/a/Cargo.toml"),
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
fn plan_responds_to_licenses_unlicensed_check_id() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
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
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_responds_to_licenses_missing_check_id() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
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
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.missing",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_responds_to_license_unlicensed_check_id() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
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
            &receipt_set_with_license_finding(
                "cargo-deny",
                "license.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

// ============================================================================
// Workspace Canonical License Resolution Tests
// ============================================================================

#[test]
fn plan_uses_workspace_package_license_as_canonical() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "Apache-2.0"

[package]
license = "MIT""#,
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
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    match &ops[0].kind {
        OpKind::TomlTransform { args, .. } => {
            // Should use workspace.package.license (Apache-2.0), not package.license (MIT)
            assert_eq!(args.as_ref().unwrap()["license"], "Apache-2.0");
        }
        _ => panic!("expected TomlTransform"),
    }
}

#[test]
fn plan_falls_back_to_package_license_when_no_workspace_package() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[package]
license = "MIT""#,
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
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    match &ops[0].kind {
        OpKind::TomlTransform { args, .. } => {
            // Should fall back to package.license (MIT)
            assert_eq!(args.as_ref().unwrap()["license"], "MIT");
        }
        _ => panic!("expected TomlTransform"),
    }
}

// ============================================================================
// Multiple Sensor Support Tests
// ============================================================================

#[test]
fn plan_accepts_cargo_deny_sensor() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
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
            &receipt_set_with_license_finding(
                "cargo-deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_accepts_deny_sensor() {
    let fixer = LicenseNormalizeFixer;
    let repo = MockRepo::new(&[
        (
            "Cargo.toml",
            r#"[workspace.package]
license = "MIT""#,
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
            &receipt_set_with_license_finding(
                "deny",
                "licenses.unlicensed",
                "crates/a/Cargo.toml",
                Some("crates/a/Cargo.toml"),
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}
