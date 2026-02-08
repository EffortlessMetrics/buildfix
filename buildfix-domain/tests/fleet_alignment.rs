//! Tests for fleet-alignment additions: fingerprint propagation, safety_counts.

use buildfix_domain::{FsRepoView, PlanContext, Planner, PlannerConfig};
use buildfix_types::receipt::ToolInfo;
use camino::Utf8PathBuf;
use fs_err as fs;
use tempfile::TempDir;

fn tool() -> ToolInfo {
    ToolInfo {
        name: "buildfix".into(),
        version: Some("0.0.0-test".into()),
        repo: None,
        commit: None,
    }
}

/// Build a minimal workspace with a resolver-v2 finding whose receipt includes a fingerprint.
fn setup_repo_with_fingerprint() -> (TempDir, Utf8PathBuf) {
    let temp = TempDir::new().unwrap();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();

    // Workspace Cargo.toml missing resolver = "2"
    let cargo_toml = r#"[workspace]
members = ["crates/a"]
"#;
    fs::write(root.join("Cargo.toml"), cargo_toml).unwrap();
    fs::create_dir_all(root.join("crates/a")).unwrap();
    fs::write(
        root.join("crates/a/Cargo.toml"),
        "[package]\nname = \"a\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    // Receipt with a fingerprint on the finding
    let receipt = serde_json::json!({
        "schema": "sensor.report.v1",
        "tool": { "name": "builddiag", "version": "1.0.0" },
        "run": {},
        "verdict": { "status": "warn", "counts": { "findings": 1, "errors": 0, "warnings": 1 } },
        "findings": [{
            "severity": "warn",
            "check_id": "workspace.resolver_v2",
            "code": "not_v2",
            "message": "resolver is not v2",
            "location": { "path": "Cargo.toml" },
            "fingerprint": "abc123-stable-key"
        }]
    });

    let artifacts = root.join("artifacts/builddiag");
    fs::create_dir_all(&artifacts).unwrap();
    fs::write(
        artifacts.join("report.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();

    (temp, root)
}

#[test]
fn fingerprint_propagates_to_finding_ref() {
    let (_temp, root) = setup_repo_with_fingerprint();
    let artifacts_dir = root.join("artifacts");

    let receipts = buildfix_receipts::load_receipts(&artifacts_dir).unwrap();
    let planner = Planner::new();
    let ctx = PlanContext {
        repo_root: root.clone(),
        artifacts_dir,
        config: PlannerConfig::default(),
    };
    let repo = FsRepoView::new(root);

    let plan = planner.plan(&ctx, &repo, &receipts, tool()).unwrap();

    assert!(!plan.ops.is_empty(), "should have at least one op");
    let findings = &plan.ops[0].rationale.findings;
    assert!(!findings.is_empty(), "op should have findings");
    assert_eq!(
        findings[0].fingerprint.as_deref(),
        Some("abc123-stable-key"),
        "fingerprint should propagate from receipt to FindingRef"
    );
}

#[test]
fn safety_counts_computed_in_summary() {
    let (_temp, root) = setup_repo_with_fingerprint();
    let artifacts_dir = root.join("artifacts");

    let receipts = buildfix_receipts::load_receipts(&artifacts_dir).unwrap();
    let planner = Planner::new();
    let ctx = PlanContext {
        repo_root: root.clone(),
        artifacts_dir,
        config: PlannerConfig::default(),
    };
    let repo = FsRepoView::new(root);

    let plan = planner.plan(&ctx, &repo, &receipts, tool()).unwrap();

    let sc = plan
        .summary
        .safety_counts
        .as_ref()
        .expect("safety_counts should be present");

    // The resolver-v2 fixer produces a Safe op
    assert!(sc.safe >= 1, "should have at least 1 safe op");
    assert_eq!(
        sc.safe + sc.guarded + sc.unsafe_count,
        plan.summary.ops_total,
        "safety counts should sum to ops_total"
    );
}
