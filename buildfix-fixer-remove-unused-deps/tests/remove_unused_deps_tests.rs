//! Integration tests for buildfix-fixer-remove-unused-deps
//!
//! These tests complement the inline tests in src/remove_unused_deps.rs

use buildfix_fixer_api::{Fixer, PlanContext, PlannerConfig, ReceiptSet, RepoView};
use buildfix_fixer_remove_unused_deps::RemoveUnusedDepsFixer;
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::{OpKind, SafetyClass};
use buildfix_types::receipt::{
    AnalysisDepth, Finding, FindingContext, Location, Provenance, ReceiptEnvelope, RunInfo,
    ToolInfo, Verdict,
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

/// Create a receipt set with an unused dependency finding
fn receipt_set_with_unused_dep(
    sensor: &str,
    check_id: &str,
    path: &str,
    toml_path: Vec<&str>,
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
            code: Some("unused_dep".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from(path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: Some(serde_json::json!({
                "toml_path": toml_path,
                "dep": toml_path.last().unwrap_or(&""),
            })),
            ..Default::default()
        }],
        capabilities: None,
        data: None,
    };

    let loaded = vec![LoadedReceipt {
        path: Utf8PathBuf::from(format!("artifacts/{}/report.json", sensor)),
        sensor_id: sensor.to_string(),
        receipt: Ok(receipt),
    }];
    ReceiptSet::from_loaded(&loaded)
}

/// Create a receipt set with evidence (confidence, provenance, context)
fn receipt_set_with_evidence(
    sensor: &str,
    check_id: &str,
    path: &str,
    toml_path: Vec<&str>,
    confidence: Option<f64>,
    provenance: Option<Provenance>,
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
            code: Some("unused_dep".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from(path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: Some(serde_json::json!({
                "toml_path": toml_path,
                "dep": toml_path.last().unwrap_or(&""),
            })),
            confidence,
            provenance,
            context,
        }],
        capabilities: None,
        data: None,
    };

    let loaded = vec![LoadedReceipt {
        path: Utf8PathBuf::from(format!("artifacts/{}/report.json", sensor)),
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
    let fixer = RemoveUnusedDepsFixer;
    let meta = fixer.meta();
    assert_eq!(meta.fix_key, "cargo.remove_unused_deps");
}

#[test]
fn fixer_meta_returns_correct_description() {
    let fixer = RemoveUnusedDepsFixer;
    let meta = fixer.meta();
    assert!(!meta.description.is_empty());
    assert!(meta.description.contains("unused"));
}

#[test]
fn fixer_meta_returns_unsafe_safety_class() {
    let fixer = RemoveUnusedDepsFixer;
    let meta = fixer.meta();
    // Base safety is Unsafe (can be promoted to Guarded with evidence)
    assert_eq!(meta.safety, SafetyClass::Unsafe);
}

#[test]
fn fixer_meta_declares_sensors() {
    let fixer = RemoveUnusedDepsFixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_sensors.is_empty());
    // Should include cargo-udeps, udeps, cargo-machete, machete
    assert!(
        meta.consumes_sensors
            .iter()
            .any(|s| *s == "cargo-udeps" || *s == "cargo-machete")
    );
}

#[test]
fn fixer_meta_declares_check_ids() {
    let fixer = RemoveUnusedDepsFixer;
    let meta = fixer.meta();
    assert!(!meta.consumes_check_ids.is_empty());
}

// ============================================================================
// Plan Generation Tests
// ============================================================================

#[test]
fn plan_returns_empty_when_no_receipts() {
    let fixer = RemoveUnusedDepsFixer;
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
fn plan_generates_unsafe_op_for_unused_dependency() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Default safety is Unsafe (without full evidence)
    assert_eq!(op.safety, SafetyClass::Unsafe);

    // Verify target
    assert_eq!(op.target.path, "crates/a/Cargo.toml");

    // Verify operation kind
    match &op.kind {
        OpKind::TomlRemove { toml_path } => {
            assert_eq!(
                toml_path,
                &vec!["dependencies".to_string(), "serde".to_string()]
            );
        }
        _ => panic!("expected TomlRemove operation"),
    }
}

#[test]
fn plan_generates_op_with_correct_rationale() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    let op = &ops[0];

    // Verify rationale
    assert!(op.rationale.description.is_some());
    assert!(!op.rationale.findings.is_empty());
    assert!(op.rationale.fix_key.contains("cargo-machete"));
}

#[test]
fn plan_handles_dev_dependencies() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dev-dependencies]
tempfile = "3""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dev-dependencies", "tempfile"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    match &ops[0].kind {
        OpKind::TomlRemove { toml_path } => {
            assert_eq!(
                toml_path,
                &vec!["dev-dependencies".to_string(), "tempfile".to_string()]
            );
        }
        _ => panic!("expected TomlRemove"),
    }
}

#[test]
fn plan_handles_build_dependencies() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[build-dependencies]
cc = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["build-dependencies", "cc"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    match &ops[0].kind {
        OpKind::TomlRemove { toml_path } => {
            assert_eq!(
                toml_path,
                &vec!["build-dependencies".to_string(), "cc".to_string()]
            );
        }
        _ => panic!("expected TomlRemove"),
    }
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

#[test]
fn plan_handles_missing_manifest_gracefully() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::empty();
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_skips_missing_dependencies() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    // Finding for a dependency that doesn't exist in the manifest
    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "tokio"], // tokio doesn't exist
            ),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_ignores_unrelated_check_ids() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "unrelated.check",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_ignores_non_manifest_paths() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[("crates/a/src/lib.rs", "// not a manifest")]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/src/lib.rs",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_skips_invalid_toml_paths() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    // Invalid toml_path (package.name is not a dependency)
    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["package", "name"],
            ),
        )
        .unwrap();

    assert!(ops.is_empty());
}

#[test]
fn plan_deduplicates_ops_for_same_manifest_and_toml_path() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);

    // Create two findings for the same dependency from different sensors
    let receipt1 = ReceiptEnvelope {
        schema: "sensor.report.v1".to_string(),
        tool: ToolInfo {
            name: "cargo-machete".to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict::default(),
        findings: vec![Finding {
            severity: Default::default(),
            check_id: Some("deps.unused_dependency".to_string()),
            code: Some("unused_dep".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from("crates/a/Cargo.toml"),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: Some(serde_json::json!({
                "toml_path": ["dependencies", "serde"],
                "dep": "serde"
            })),
            ..Default::default()
        }],
        capabilities: None,
        data: None,
    };

    let receipt2 = ReceiptEnvelope {
        schema: "sensor.report.v1".to_string(),
        tool: ToolInfo {
            name: "cargo-udeps".to_string(),
            version: None,
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict::default(),
        findings: vec![Finding {
            severity: Default::default(),
            check_id: Some("deps.unused_dependency".to_string()),
            code: Some("unused_dep".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from("crates/a/Cargo.toml"),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: Some(serde_json::json!({
                "toml_path": ["dependencies", "serde"],
                "dep": "serde"
            })),
            ..Default::default()
        }],
        capabilities: None,
        data: None,
    };

    let loaded = vec![
        LoadedReceipt {
            path: Utf8PathBuf::from("artifacts/cargo-machete/report.json"),
            sensor_id: "cargo-machete".to_string(),
            receipt: Ok(receipt1),
        },
        LoadedReceipt {
            path: Utf8PathBuf::from("artifacts/cargo-udeps/report.json"),
            sensor_id: "cargo-udeps".to_string(),
            receipt: Ok(receipt2),
        },
    ];
    let receipt_set = ReceiptSet::from_loaded(&loaded);

    let ctx = plan_context();
    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();

    // Should deduplicate to a single op
    assert_eq!(ops.len(), 1);
    // But rationale should include both findings
    assert_eq!(ops[0].rationale.findings.len(), 2);
}

// ============================================================================
// Safety Promotion Tests
// ============================================================================

#[test]
fn plan_promotes_to_guarded_with_full_evidence() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    // High confidence + full analysis + tool agreement → Guarded
    let receipt_set = receipt_set_with_evidence(
        "cargo-machete",
        "deps.unused_dependency",
        "crates/a/Cargo.toml",
        vec!["dependencies", "serde"],
        Some(0.95), // High confidence (≥0.9)
        Some(Provenance {
            method: "dead_code_analysis".to_string(),
            tools: vec!["cargo-udeps".to_string(), "cargo-machete".to_string()],
            agreement: true, // Tool agreement
            evidence_chain: vec![],
        }),
        Some(FindingContext {
            workspace: None,
            analysis_depth: Some(AnalysisDepth::Full), // Full analysis
        }),
    );

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].safety, SafetyClass::Guarded);
}

#[test]
fn plan_promotes_to_guarded_with_deep_analysis() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    // High confidence + deep analysis + tool agreement → Guarded
    let receipt_set = receipt_set_with_evidence(
        "cargo-machete",
        "deps.unused_dependency",
        "crates/a/Cargo.toml",
        vec!["dependencies", "serde"],
        Some(0.92), // High confidence (≥0.9)
        Some(Provenance {
            method: "dead_code_analysis".to_string(),
            tools: vec!["cargo-udeps".to_string(), "cargo-machete".to_string()],
            agreement: true, // Tool agreement
            evidence_chain: vec![],
        }),
        Some(FindingContext {
            workspace: None,
            analysis_depth: Some(AnalysisDepth::Deep), // Deep analysis also qualifies
        }),
    );

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].safety, SafetyClass::Guarded);
}

#[test]
fn plan_remains_unsafe_with_low_confidence() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    // Low confidence (< 0.9) → Unsafe, even with other evidence
    let receipt_set = receipt_set_with_evidence(
        "cargo-machete",
        "deps.unused_dependency",
        "crates/a/Cargo.toml",
        vec!["dependencies", "serde"],
        Some(0.75), // Low confidence (< 0.9)
        Some(Provenance {
            method: "dead_code_analysis".to_string(),
            tools: vec!["cargo-udeps".to_string(), "cargo-machete".to_string()],
            agreement: true,
            evidence_chain: vec![],
        }),
        Some(FindingContext {
            workspace: None,
            analysis_depth: Some(AnalysisDepth::Full),
        }),
    );

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].safety, SafetyClass::Unsafe);
}

#[test]
fn plan_remains_unsafe_without_tool_agreement() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    // No tool agreement → Unsafe, even with high confidence and full analysis
    let receipt_set = receipt_set_with_evidence(
        "cargo-machete",
        "deps.unused_dependency",
        "crates/a/Cargo.toml",
        vec!["dependencies", "serde"],
        Some(0.95), // High confidence
        Some(Provenance {
            method: "dead_code_analysis".to_string(),
            tools: vec!["cargo-udeps".to_string()],
            agreement: false, // No tool agreement
            evidence_chain: vec![],
        }),
        Some(FindingContext {
            workspace: None,
            analysis_depth: Some(AnalysisDepth::Full),
        }),
    );

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].safety, SafetyClass::Unsafe);
}

#[test]
fn plan_remains_unsafe_with_shallow_analysis() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    // Shallow analysis → Unsafe, even with high confidence and tool agreement
    let receipt_set = receipt_set_with_evidence(
        "cargo-machete",
        "deps.unused_dependency",
        "crates/a/Cargo.toml",
        vec!["dependencies", "serde"],
        Some(0.95), // High confidence
        Some(Provenance {
            method: "dead_code_analysis".to_string(),
            tools: vec!["cargo-udeps".to_string(), "cargo-machete".to_string()],
            agreement: true,
            evidence_chain: vec![],
        }),
        Some(FindingContext {
            workspace: None,
            analysis_depth: Some(AnalysisDepth::Shallow), // Shallow analysis
        }),
    );

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].safety, SafetyClass::Unsafe);
}

#[test]
fn plan_remains_unsafe_with_missing_analysis_depth() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    // High confidence + tool agreement, but missing analysis_depth → Unsafe
    let receipt_set = receipt_set_with_evidence(
        "cargo-machete",
        "deps.unused_dependency",
        "crates/a/Cargo.toml",
        vec!["dependencies", "serde"],
        Some(0.95), // High confidence
        Some(Provenance {
            method: "dead_code_analysis".to_string(),
            tools: vec!["cargo-udeps".to_string(), "cargo-machete".to_string()],
            agreement: true,
            evidence_chain: vec![],
        }),
        None, // Missing analysis_depth
    );

    let ops = fixer.plan(&ctx, &repo, &receipt_set).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].safety, SafetyClass::Unsafe);
}

// ============================================================================
// Multiple Check ID Support Tests
// ============================================================================

#[test]
fn plan_responds_to_deps_unused_dependency_check_id() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_responds_to_deps_unused_dependencies_check_id() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependencies",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_responds_to_cargo_unused_dependency_check_id() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "cargo.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_responds_to_udeps_unused_dependency_check_id() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-udeps",
                "udeps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_responds_to_machete_unused_dependency_check_id() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "machete.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

// ============================================================================
// Multiple Sensor Support Tests
// ============================================================================

#[test]
fn plan_accepts_cargo_udeps_sensor() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-udeps",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_accepts_udeps_sensor() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "udeps",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_accepts_cargo_machete_sensor() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

#[test]
fn plan_accepts_machete_sensor() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[dependencies]
serde = "1.0""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["dependencies", "serde"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
}

// ============================================================================
// Target Configuration Tests
// ============================================================================

#[test]
fn plan_handles_target_cfg_dependencies() {
    let fixer = RemoveUnusedDepsFixer;
    let repo = MockRepo::new(&[(
        "crates/a/Cargo.toml",
        r#"[package]
name = "a"

[target.'cfg(windows)'.dependencies]
winapi = "0.3""#,
    )]);
    let ctx = plan_context();

    let ops = fixer
        .plan(
            &ctx,
            &repo,
            &receipt_set_with_unused_dep(
                "cargo-machete",
                "deps.unused_dependency",
                "crates/a/Cargo.toml",
                vec!["target", "cfg(windows)", "dependencies", "winapi"],
            ),
        )
        .unwrap();

    assert_eq!(ops.len(), 1);
    match &ops[0].kind {
        OpKind::TomlRemove { toml_path } => {
            assert_eq!(
                toml_path,
                &vec![
                    "target".to_string(),
                    "cfg(windows)".to_string(),
                    "dependencies".to_string(),
                    "winapi".to_string()
                ]
            );
        }
        _ => panic!("expected TomlRemove"),
    }
}
