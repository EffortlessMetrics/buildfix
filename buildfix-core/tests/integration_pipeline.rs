//! End-to-end integration tests for the buildfix pipeline.
//!
//! These tests exercise the full pipeline: receipt loading -> domain planning
//! -> edit engine -> artifact output, using the public API from buildfix-core.
//!
//! All tests use filesystem-backed receipt sources and real temp directories
//! to verify the pipeline works as it would in production.

use buildfix_core::adapters::FsReceiptSource;
use buildfix_core::pipeline::{PlanOutcome, run_apply, run_plan, write_plan_artifacts};
use buildfix_core::ports::{GitPort, WritePort};
use buildfix_core::settings::{ApplySettings, PlanSettings, RunMode};
use buildfix_types::ops::SafetyClass;
use buildfix_types::receipt::ToolInfo;
use buildfix_types::wire::PlanV1;
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;
use tempfile::TempDir;

// =============================================================================
// Test infrastructure
// =============================================================================

/// A git port that returns no git info (simulates running outside a git repo).
struct NullGitPort;

impl GitPort for NullGitPort {
    fn head_sha(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn is_dirty(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<bool>> {
        Ok(Some(false))
    }
}

/// A write port backed by the real filesystem.
struct FsWritePort;

impl WritePort for FsWritePort {
    fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
        Ok(())
    }

    fn create_dir_all(&self, path: &Utf8Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(path)?;
        Ok(())
    }
}

fn tool_info() -> ToolInfo {
    ToolInfo {
        name: "buildfix".to_string(),
        version: Some("test".to_string()),
        repo: None,
        commit: None,
    }
}

fn default_plan_settings(root: &Utf8Path, artifacts_dir: &Utf8Path) -> PlanSettings {
    PlanSettings {
        repo_root: root.to_path_buf(),
        artifacts_dir: artifacts_dir.to_path_buf(),
        out_dir: artifacts_dir.join("buildfix"),
        allow: Vec::new(),
        deny: Vec::new(),
        allow_guarded: false,
        allow_unsafe: false,
        allow_dirty: false,
        max_ops: None,
        max_files: None,
        max_patch_bytes: None,
        params: HashMap::new(),
        require_clean_hashes: true,
        git_head_precondition: false,
        backup_suffix: ".buildfix.bak".to_string(),
        mode: RunMode::Standalone,
    }
}

fn default_apply_settings(root: &Utf8Path, out_dir: &Utf8Path) -> ApplySettings {
    ApplySettings {
        repo_root: root.to_path_buf(),
        out_dir: out_dir.to_path_buf(),
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        allow_dirty: true, // no real git repo, so allow dirty
        params: HashMap::new(),
        auto_commit: false,
        commit_message: None,
        backup_enabled: false,
        backup_suffix: ".buildfix.bak".to_string(),
        mode: RunMode::Standalone,
    }
}

/// Normalize line endings for cross-platform comparison.
fn normalize_line_endings(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Creates a temp repo with the given workspace Cargo.toml, optional crate
/// subdirectories, and sensor receipts. Returns (TempDir, root, artifacts_dir).
struct TestRepo {
    _temp: TempDir,
    root: Utf8PathBuf,
    artifacts_dir: Utf8PathBuf,
}

impl TestRepo {
    fn new() -> Self {
        let temp = TempDir::new().expect("create temp dir");
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
            .expect("temp dir path should be valid UTF-8");
        let artifacts_dir = root.join("artifacts");
        std::fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");
        Self {
            _temp: temp,
            root,
            artifacts_dir,
        }
    }

    fn write_file(&self, rel_path: &str, contents: &str) {
        let path = self.root.join(rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(&path, contents).expect("write file");
    }

    fn read_file(&self, rel_path: &str) -> String {
        let path = self.root.join(rel_path);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read file {}: {}", rel_path, e))
    }

    fn write_receipt(&self, sensor_name: &str, receipt_json: &str) {
        let sensor_dir = self.artifacts_dir.join(sensor_name);
        std::fs::create_dir_all(&sensor_dir).expect("create sensor dir");
        std::fs::write(sensor_dir.join("report.json"), receipt_json).expect("write receipt");
    }
}

/// Run plan and write artifacts in one step (common pattern in plan-then-apply tests).
fn plan_and_write(repo: &TestRepo) -> (PlanOutcome, Utf8PathBuf) {
    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let plan_settings = default_plan_settings(&repo.root, &repo.artifacts_dir);
    let out_dir = plan_settings.out_dir.clone();

    let plan_outcome = run_plan(&plan_settings, &receipts_port, &NullGitPort, tool_info())
        .expect("run_plan should succeed");

    write_plan_artifacts(&plan_outcome, &out_dir, &FsWritePort)
        .expect("write_plan_artifacts should succeed");

    (plan_outcome, out_dir)
}

/// A builddiag receipt that reports resolver_v2 is missing.
const RESOLVER_V2_RECEIPT: &str = r#"{
    "schema": "builddiag.report.v1",
    "tool": { "name": "builddiag", "version": "0.0.0" },
    "verdict": {
        "status": "fail",
        "counts": { "findings": 1, "errors": 1, "warnings": 0 }
    },
    "findings": [
        {
            "severity": "error",
            "check_id": "workspace.resolver_v2",
            "code": "not_v2",
            "message": "workspace resolver is not 2",
            "location": { "path": "Cargo.toml", "line": 1, "column": 1 }
        }
    ]
}"#;

/// A depguard receipt that reports a path dep missing version.
fn path_dep_version_receipt(
    manifest_path: &str,
    dep_name: &str,
    dep_path: &str,
    line: u64,
) -> String {
    serde_json::json!({
        "schema": "depguard.report.v1",
        "tool": { "name": "depguard", "version": "0.0.0" },
        "verdict": {
            "status": "fail",
            "counts": { "findings": 1, "errors": 1, "warnings": 0 }
        },
        "findings": [
            {
                "severity": "error",
                "check_id": "deps.path_requires_version",
                "code": "missing_version",
                "message": "path dependency missing version",
                "location": { "path": manifest_path, "line": line, "column": 1 },
                "data": {
                    "dep": dep_name,
                    "dep_path": dep_path,
                    "toml_path": ["dependencies", dep_name]
                }
            }
        ]
    })
    .to_string()
}

/// A builddiag receipt that reports MSRV mismatch (triggers Guarded fixer).
fn msrv_mismatch_receipt(crate_path: &str, crate_msrv: &str, workspace_msrv: &str) -> String {
    serde_json::json!({
        "schema": "builddiag.report.v1",
        "tool": { "name": "builddiag", "version": "0.0.0" },
        "verdict": {
            "status": "fail",
            "counts": { "findings": 1, "errors": 1, "warnings": 0 }
        },
        "findings": [
            {
                "severity": "error",
                "check_id": "rust.msrv_consistent",
                "code": "msrv_mismatch",
                "message": "crate MSRV does not match workspace",
                "location": { "path": crate_path, "line": 5, "column": 1 },
                "data": {
                    "crate_msrv": crate_msrv,
                    "workspace_msrv": workspace_msrv
                }
            }
        ]
    })
    .to_string()
}

/// Set up a repo with a workspace missing resolver = "2" (triggers Safe fixer).
fn setup_resolver_v2_repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.write_file("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n");
    repo.write_file(
        "crates/a/Cargo.toml",
        "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    repo.write_receipt("builddiag", RESOLVER_V2_RECEIPT);
    repo
}

/// Set up a repo with an MSRV mismatch (triggers Guarded fixer).
fn setup_msrv_mismatch_repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.write_file(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/a\"]\nresolver = \"2\"\n\n[workspace.package]\nrust-version = \"1.70\"\n",
    );
    repo.write_file(
        "crates/a/Cargo.toml",
        "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\nrust-version = \"1.65\"\n",
    );
    repo.write_receipt(
        "builddiag",
        &msrv_mismatch_receipt("crates/a/Cargo.toml", "1.65", "1.70"),
    );
    repo
}

/// Set up a repo with both resolver_v2 and path dep version issues.
fn setup_multi_fixer_repo() -> TestRepo {
    let repo = TestRepo::new();
    repo.write_file(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n",
    );
    repo.write_file(
        "crates/a/Cargo.toml",
        "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\ncrate-b = { path = \"../b\" }\n",
    );
    repo.write_file(
        "crates/b/Cargo.toml",
        "[package]\nname = \"crate-b\"\nversion = \"0.2.0\"\nedition = \"2021\"\n",
    );
    repo.write_receipt("builddiag", RESOLVER_V2_RECEIPT);
    repo.write_receipt(
        "depguard",
        &path_dep_version_receipt("crates/a/Cargo.toml", "crate-b", "../b", 7),
    );
    repo
}

// =============================================================================
// Test: plan resolver_v2 from receipt
// =============================================================================

#[test]
fn test_plan_resolver_v2_from_receipt() {
    let repo = setup_resolver_v2_repo();

    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let settings = default_plan_settings(&repo.root, &repo.artifacts_dir);

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info())
        .expect("run_plan should succeed for resolver_v2 receipt");

    // Plan should have exactly 1 op
    assert_eq!(
        outcome.plan.ops.len(),
        1,
        "expected exactly 1 op, got {}",
        outcome.plan.ops.len()
    );

    let op = &outcome.plan.ops[0];

    // Op kind should be a TomlTransform with rule_id ensure_workspace_resolver_v2
    match &op.kind {
        buildfix_types::ops::OpKind::TomlTransform { rule_id, .. } => {
            assert_eq!(rule_id, "ensure_workspace_resolver_v2");
        }
        other => panic!(
            "expected TomlTransform, got {:?}",
            std::mem::discriminant(other)
        ),
    }

    // Safety should be safe
    assert_eq!(op.safety, SafetyClass::Safe);

    // Op should not be blocked
    assert!(!op.blocked);

    // Target should be Cargo.toml
    assert_eq!(op.target.path, "Cargo.toml");

    // Patch should contain +resolver = "2"
    let patch = normalize_line_endings(&outcome.patch);
    assert!(
        patch.contains("+resolver = \"2\""),
        "patch should contain '+resolver = \"2\"', got:\n{}",
        patch
    );

    // No policy block
    assert!(!outcome.policy_block);
}

// =============================================================================
// Test: plan with multiple fixers
// =============================================================================

#[test]
fn test_plan_multiple_fixers() {
    let repo = setup_multi_fixer_repo();

    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let settings = default_plan_settings(&repo.root, &repo.artifacts_dir);

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info())
        .expect("run_plan should succeed for multi-fixer scenario");

    // Should have at least 2 ops (resolver_v2 + path_dep_version)
    assert!(
        outcome.plan.ops.len() >= 2,
        "expected at least 2 ops, got {}",
        outcome.plan.ops.len()
    );

    // Collect fix keys
    let fix_keys: Vec<&str> = outcome
        .plan
        .ops
        .iter()
        .map(|o| o.rationale.fix_key.as_str())
        .collect();

    // Should contain both fix keys
    assert!(
        fix_keys.iter().any(|k| k.contains("workspace.resolver_v2")),
        "missing resolver_v2 fix key in {:?}",
        fix_keys
    );
    assert!(
        fix_keys
            .iter()
            .any(|k| k.contains("deps.path_requires_version")),
        "missing path_dep_version fix key in {:?}",
        fix_keys
    );

    // Ops should target different files
    let targets: Vec<&str> = outcome
        .plan
        .ops
        .iter()
        .map(|o| o.target.path.as_str())
        .collect();
    assert!(
        targets.contains(&"Cargo.toml"),
        "should have Cargo.toml target"
    );
    assert!(
        targets.contains(&"crates/a/Cargo.toml"),
        "should have crates/a/Cargo.toml target"
    );

    // All ops from these fixers should be Safe
    for op in &outcome.plan.ops {
        assert_eq!(
            op.safety,
            SafetyClass::Safe,
            "op with fix_key '{}' should be Safe, got {:?}",
            op.rationale.fix_key,
            op.safety
        );
    }

    // Summary should reflect the ops
    assert_eq!(
        outcome.plan.summary.ops_total,
        outcome.plan.ops.len() as u64
    );
    assert_eq!(outcome.plan.summary.ops_blocked, 0);
    assert!(outcome.plan.summary.files_touched >= 2);
}

// =============================================================================
// Test: apply produces correct files
// =============================================================================

#[test]
fn test_apply_produces_correct_files() {
    let repo = setup_resolver_v2_repo();

    let (plan_outcome, out_dir) = plan_and_write(&repo);

    // Verify plan.json was written
    assert!(
        out_dir.join("plan.json").exists(),
        "plan.json should exist after write_plan_artifacts"
    );

    // Verify plan has the expected op before apply
    assert_eq!(
        plan_outcome.plan.ops.len(),
        1,
        "plan should have exactly 1 op"
    );

    // Run apply (real, non-dry-run)
    let apply_settings = default_apply_settings(&repo.root, &out_dir);
    let apply_outcome =
        run_apply(&apply_settings, &NullGitPort, tool_info()).expect("run_apply should succeed");

    // Apply should have applied 1 op
    assert_eq!(
        apply_outcome.apply.summary.applied, 1,
        "expected 1 applied op, got {}",
        apply_outcome.apply.summary.applied
    );
    assert_eq!(apply_outcome.apply.summary.failed, 0, "no ops should fail");
    assert_eq!(
        apply_outcome.apply.summary.blocked, 0,
        "no ops should be blocked"
    );

    // The Cargo.toml should now contain resolver = "2"
    let cargo_toml = repo.read_file("Cargo.toml");
    let normalized = normalize_line_endings(&cargo_toml);
    assert!(
        normalized.contains("resolver = \"2\""),
        "Cargo.toml should contain 'resolver = \"2\"' after apply, got:\n{}",
        normalized
    );

    // Original workspace structure should be preserved (not clobbered)
    assert!(
        normalized.contains("[workspace]"),
        "Cargo.toml should still contain [workspace] section"
    );
    assert!(
        normalized.contains("members"),
        "Cargo.toml should still contain members key"
    );

    // Preconditions should have been verified
    assert!(
        apply_outcome.apply.preconditions.verified,
        "preconditions should be verified"
    );

    // No policy block
    assert!(!apply_outcome.policy_block);
}

// =============================================================================
// Test: precondition mismatch blocks apply
// =============================================================================

#[test]
fn test_precondition_mismatch_blocks_apply() {
    let repo = setup_resolver_v2_repo();

    let (plan_outcome, out_dir) = plan_and_write(&repo);

    // Ensure preconditions were attached
    assert!(
        !plan_outcome.plan.preconditions.files.is_empty(),
        "plan should have file preconditions"
    );

    // Now modify the Cargo.toml to break the SHA256 precondition
    repo.write_file(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/a\"]\n# modified after plan\n",
    );

    // Run apply (real, non-dry-run)
    let apply_settings = default_apply_settings(&repo.root, &out_dir);
    let apply_outcome = run_apply(&apply_settings, &NullGitPort, tool_info())
        .expect("run_apply should succeed even with mismatch (returns outcome, not error)");

    // Apply should detect precondition mismatch
    assert!(
        !apply_outcome.apply.preconditions.verified,
        "preconditions should NOT be verified after modification"
    );

    // There should be mismatches reported
    assert!(
        !apply_outcome.apply.preconditions.mismatches.is_empty(),
        "should have precondition mismatches"
    );

    // The mismatch should be for Cargo.toml
    let mismatch = &apply_outcome.apply.preconditions.mismatches[0];
    assert_eq!(mismatch.path, "Cargo.toml");

    // All ops should be blocked (zero applied)
    assert_eq!(
        apply_outcome.apply.summary.applied, 0,
        "no ops should have been applied when preconditions fail"
    );

    // Policy block should be set (exit code 2 behavior)
    assert!(
        apply_outcome.policy_block,
        "precondition mismatch should trigger policy block"
    );
}

// =============================================================================
// Test: deterministic output
// =============================================================================

#[test]
fn test_deterministic_output() {
    // Create two identical repos and run the plan twice.
    // Outputs should be byte-identical.

    let make_repo = || {
        let repo = TestRepo::new();
        repo.write_file("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n");
        repo.write_file(
            "crates/a/Cargo.toml",
            "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );
        repo.write_receipt("builddiag", RESOLVER_V2_RECEIPT);
        repo
    };

    let run_pipeline = |repo: &TestRepo| {
        let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
        let settings = default_plan_settings(&repo.root, &repo.artifacts_dir);
        run_plan(&settings, &receipts_port, &NullGitPort, tool_info())
            .expect("run_plan should succeed for determinism test")
    };

    let repo1 = make_repo();
    let repo2 = make_repo();

    let outcome1 = run_pipeline(&repo1);
    let outcome2 = run_pipeline(&repo2);

    // Serialize plans to wire format for comparison (this is what gets written to disk)
    let plan_wire1 = PlanV1::try_from(&outcome1.plan).expect("convert plan1 to wire format");
    let plan_wire2 = PlanV1::try_from(&outcome2.plan).expect("convert plan2 to wire format");

    let plan_json1 = serde_json::to_string_pretty(&plan_wire1).expect("serialize plan1 to JSON");
    let plan_json2 = serde_json::to_string_pretty(&plan_wire2).expect("serialize plan2 to JSON");

    // Normalize dynamic fields (repo root) for comparison.
    // JSON serialization escapes backslashes, so we must also replace the
    // escaped form (e.g., "C:\\Users\\..." becomes "C:\\\\Users\\\\...").
    let normalize = |json: &str, root: &str| -> String {
        let root_fwd = root.replace('\\', "/");
        let root_escaped = root.replace('\\', "\\\\");
        json.replace(&root_escaped, "<REPO_ROOT>")
            .replace(&root_fwd, "<REPO_ROOT>")
            .replace(root, "<REPO_ROOT>")
    };

    let normalized1 = normalize(&plan_json1, repo1.root.as_str());
    let normalized2 = normalize(&plan_json2, repo2.root.as_str());

    assert_eq!(
        normalized1, normalized2,
        "plan output should be byte-identical across runs"
    );

    // Patches should also be identical (after normalizing paths)
    let patch1 = normalize_line_endings(&outcome1.patch);
    let patch2 = normalize_line_endings(&outcome2.patch);

    assert_eq!(
        patch1, patch2,
        "patch output should be byte-identical across runs"
    );

    // Op IDs should be deterministic (UUID v5 based on content)
    assert_eq!(
        outcome1.plan.ops[0].id, outcome2.plan.ops[0].id,
        "op IDs should be deterministic"
    );
}

// =============================================================================
// Test: multi-fixer apply modifies correct files
// =============================================================================

#[test]
fn test_multi_fixer_apply_modifies_correct_files() {
    let repo = setup_multi_fixer_repo();

    let (plan_outcome, out_dir) = plan_and_write(&repo);

    assert!(
        plan_outcome.plan.ops.len() >= 2,
        "expected at least 2 ops, got {}",
        plan_outcome.plan.ops.len()
    );

    // Run apply
    let apply_settings = default_apply_settings(&repo.root, &out_dir);
    let apply_outcome = run_apply(&apply_settings, &NullGitPort, tool_info())
        .expect("run_apply should succeed for multi-fixer scenario");

    // All ops should have been applied
    assert_eq!(apply_outcome.apply.summary.failed, 0, "no ops should fail");
    assert!(
        apply_outcome.apply.summary.applied >= 2,
        "expected at least 2 applied ops, got {}",
        apply_outcome.apply.summary.applied
    );

    // Root Cargo.toml should have resolver = "2"
    let root_toml = normalize_line_endings(&repo.read_file("Cargo.toml"));
    assert!(
        root_toml.contains("resolver = \"2\""),
        "root Cargo.toml should have resolver = \"2\", got:\n{}",
        root_toml
    );

    // crates/a/Cargo.toml should have a version field on the path dep
    let crate_a_toml = normalize_line_endings(&repo.read_file("crates/a/Cargo.toml"));
    assert!(
        crate_a_toml.contains("version = \"0.2.0\"")
            || crate_a_toml.contains("version = \"=0.2.0\""),
        "crates/a/Cargo.toml should have version on path dep, got:\n{}",
        crate_a_toml
    );
}

// =============================================================================
// Test: empty receipts produce no ops
// =============================================================================

#[test]
fn test_empty_receipts_produce_no_ops() {
    let repo = TestRepo::new();

    // Valid workspace, no issues
    repo.write_file(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/a\"]\nresolver = \"2\"\n",
    );
    repo.write_file(
        "crates/a/Cargo.toml",
        "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    // Receipt with no findings
    repo.write_receipt(
        "builddiag",
        r#"{
            "schema": "builddiag.report.v1",
            "tool": { "name": "builddiag", "version": "0.0.0" },
            "verdict": {
                "status": "pass",
                "counts": { "findings": 0, "errors": 0, "warnings": 0 }
            },
            "findings": []
        }"#,
    );

    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let settings = default_plan_settings(&repo.root, &repo.artifacts_dir);

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info())
        .expect("run_plan should succeed with empty receipts");

    assert_eq!(
        outcome.plan.ops.len(),
        0,
        "no findings should produce no ops"
    );
    assert!(outcome.patch.is_empty(), "patch should be empty");
    assert!(!outcome.policy_block);
}

// =============================================================================
// Test: plan artifacts are valid JSON
// =============================================================================

#[test]
fn test_plan_artifacts_are_valid_json() {
    let repo = setup_resolver_v2_repo();

    let (_plan_outcome, out_dir) = plan_and_write(&repo);

    // Read and validate plan.json
    let plan_json =
        std::fs::read_to_string(out_dir.join("plan.json")).expect("read plan.json from output dir");
    let plan_value: serde_json::Value =
        serde_json::from_str(&plan_json).expect("plan.json should be valid JSON");

    // Check required fields
    assert!(plan_value.get("schema").is_some(), "plan.json needs schema");
    assert!(plan_value.get("ops").is_some(), "plan.json needs ops");
    assert!(
        plan_value.get("summary").is_some(),
        "plan.json needs summary"
    );
    assert!(plan_value.get("tool").is_some(), "plan.json needs tool");

    // Read and validate report.json
    let report_json = std::fs::read_to_string(out_dir.join("report.json"))
        .expect("read report.json from output dir");
    let report_value: serde_json::Value =
        serde_json::from_str(&report_json).expect("report.json should be valid JSON");

    assert!(
        report_value.get("schema").is_some(),
        "report.json needs schema"
    );
    assert!(
        report_value.get("verdict").is_some(),
        "report.json needs verdict"
    );

    // patch.diff should exist
    let patch = std::fs::read_to_string(out_dir.join("patch.diff"))
        .expect("read patch.diff from output dir");
    assert!(!patch.is_empty(), "patch.diff should not be empty");
}

// =============================================================================
// Test: deny list blocks specific fix keys
// =============================================================================

#[test]
fn test_deny_list_blocks_matching_ops() {
    let repo = setup_resolver_v2_repo();

    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let mut settings = default_plan_settings(&repo.root, &repo.artifacts_dir);

    // Deny the resolver_v2 fix by its source/check_id/code pattern
    settings.deny = vec!["builddiag/workspace.resolver_v2/*".to_string()];

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info())
        .expect("run_plan should succeed even with deny list");

    // Should still have 1 op, but it should be blocked
    assert_eq!(
        outcome.plan.ops.len(),
        1,
        "denied op should still appear in plan"
    );

    let op = &outcome.plan.ops[0];
    assert!(op.blocked, "op should be blocked by deny list");
    assert_eq!(
        op.blocked_reason.as_deref(),
        Some("denied by policy"),
        "blocked_reason should indicate deny policy"
    );

    // Summary should show 1 blocked op
    assert_eq!(outcome.plan.summary.ops_blocked, 1);

    // policy_block should be true since all ops are blocked
    assert!(
        outcome.policy_block,
        "plan with all ops blocked should set policy_block"
    );
}

// =============================================================================
// Test: deny list blocks only matching ops in multi-fixer plan
// =============================================================================

#[test]
fn test_deny_list_blocks_selectively() {
    let repo = setup_multi_fixer_repo();

    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let mut settings = default_plan_settings(&repo.root, &repo.artifacts_dir);

    // Deny only the resolver_v2 fix; path_dep_version should remain unblocked
    settings.deny = vec!["builddiag/workspace.resolver_v2/*".to_string()];

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info())
        .expect("run_plan should succeed with selective deny");

    // Should have at least 2 ops
    assert!(
        outcome.plan.ops.len() >= 2,
        "expected at least 2 ops, got {}",
        outcome.plan.ops.len()
    );

    // The resolver_v2 op should be blocked
    let resolver_op = outcome
        .plan
        .ops
        .iter()
        .find(|o| o.rationale.fix_key.contains("workspace.resolver_v2"))
        .expect("should have resolver_v2 op");
    assert!(
        resolver_op.blocked,
        "resolver_v2 op should be blocked by deny list"
    );

    // The path_dep_version op should NOT be blocked
    let path_dep_op = outcome
        .plan
        .ops
        .iter()
        .find(|o| o.rationale.fix_key.contains("path_requires_version"))
        .expect("should have path_dep_version op");
    assert!(
        !path_dep_op.blocked,
        "path_dep_version op should NOT be blocked"
    );

    // Summary should show exactly 1 blocked op
    assert_eq!(outcome.plan.summary.ops_blocked, 1);
}

// =============================================================================
// Test: guarded ops are blocked by default, allowed with --allow-guarded
// =============================================================================

#[test]
fn test_guarded_ops_blocked_by_default_allowed_with_flag() {
    let repo = setup_msrv_mismatch_repo();

    // First: plan without allow_guarded - guarded op should appear unblocked in plan
    // (blocking happens at apply time for safety class)
    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let plan_settings = default_plan_settings(&repo.root, &repo.artifacts_dir);
    let out_dir = plan_settings.out_dir.clone();

    let plan_outcome = run_plan(&plan_settings, &receipts_port, &NullGitPort, tool_info())
        .expect("run_plan should succeed for guarded fixer");

    // Should have at least 1 op for the MSRV normalize.
    // The fix_key is source/check_id/code format: "builddiag/rust.msrv_consistent/msrv_mismatch"
    let msrv_op = plan_outcome
        .plan
        .ops
        .iter()
        .find(|o| o.rationale.fix_key.contains("msrv_consistent"))
        .expect("should have MSRV normalize op in plan");
    assert_eq!(
        msrv_op.safety,
        SafetyClass::Guarded,
        "MSRV normalize op should have Guarded safety class"
    );

    // Write plan artifacts for apply
    write_plan_artifacts(&plan_outcome, &out_dir, &FsWritePort)
        .expect("write_plan_artifacts should succeed");

    // Apply WITHOUT allow_guarded -- guarded ops should be blocked at apply time
    let apply_settings = default_apply_settings(&repo.root, &out_dir);
    assert!(
        !apply_settings.allow_guarded,
        "default apply_settings should not allow guarded"
    );

    let apply_outcome = run_apply(&apply_settings, &NullGitPort, tool_info())
        .expect("run_apply should succeed (guarded ops blocked, not errored)");

    // Guarded ops should be blocked at apply time
    let guarded_blocked = apply_outcome
        .apply
        .results
        .iter()
        .any(|r| r.status == buildfix_types::apply::ApplyStatus::Blocked);
    assert!(
        guarded_blocked,
        "guarded ops should be blocked when allow_guarded is false"
    );

    // The file should NOT have been modified
    let crate_toml = normalize_line_endings(&repo.read_file("crates/a/Cargo.toml"));
    assert!(
        crate_toml.contains("rust-version = \"1.65\""),
        "crate MSRV should still be 1.65 when guarded is not allowed, got:\n{}",
        crate_toml
    );

    // Now apply WITH allow_guarded -- need fresh plan since file is unchanged
    let mut apply_settings_guarded = default_apply_settings(&repo.root, &out_dir);
    apply_settings_guarded.allow_guarded = true;

    let apply_outcome_guarded = run_apply(&apply_settings_guarded, &NullGitPort, tool_info())
        .expect("run_apply should succeed with allow_guarded");

    // Check that at least one MSRV op was applied
    assert!(
        apply_outcome_guarded.apply.summary.applied >= 1,
        "at least 1 guarded op should be applied with allow_guarded, got applied={}",
        apply_outcome_guarded.apply.summary.applied
    );

    // The crate MSRV should now be updated to workspace value
    let crate_toml_after = normalize_line_endings(&repo.read_file("crates/a/Cargo.toml"));
    assert!(
        crate_toml_after.contains("rust-version = \"1.70\""),
        "crate MSRV should be updated to 1.70 after guarded apply, got:\n{}",
        crate_toml_after
    );
}

// =============================================================================
// Test: dry-run apply does not modify files
// =============================================================================

#[test]
fn test_dry_run_does_not_modify_files() {
    let repo = setup_resolver_v2_repo();

    let (_plan_outcome, out_dir) = plan_and_write(&repo);

    // Capture file content before apply
    let cargo_toml_before = repo.read_file("Cargo.toml");

    // Run apply in dry-run mode
    let mut apply_settings = default_apply_settings(&repo.root, &out_dir);
    apply_settings.dry_run = true;

    let apply_outcome = run_apply(&apply_settings, &NullGitPort, tool_info())
        .expect("dry-run apply should succeed");

    // Dry-run should not set policy_block (per check_policy_block logic)
    assert!(
        !apply_outcome.policy_block,
        "dry-run should never set policy_block"
    );

    // The file should NOT have been modified
    let cargo_toml_after = repo.read_file("Cargo.toml");
    assert_eq!(
        cargo_toml_before, cargo_toml_after,
        "dry-run should not modify Cargo.toml"
    );
}

// =============================================================================
// Test: no receipts directory produces no ops
// =============================================================================

#[test]
fn test_no_receipt_files_produce_no_ops() {
    let repo = TestRepo::new();

    // Valid workspace with no issues and no receipts at all
    repo.write_file(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/a\"]\nresolver = \"2\"\n",
    );
    repo.write_file(
        "crates/a/Cargo.toml",
        "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    // No receipts written -- artifacts dir is empty
    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let settings = default_plan_settings(&repo.root, &repo.artifacts_dir);

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info())
        .expect("run_plan should succeed with no receipts");

    assert_eq!(
        outcome.plan.ops.len(),
        0,
        "no receipt files should produce no ops"
    );
    assert!(!outcome.policy_block);
}

// =============================================================================
// Test: each op has a non-empty deterministic ID
// =============================================================================

#[test]
fn test_all_ops_have_deterministic_ids() {
    let repo = setup_multi_fixer_repo();

    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let settings = default_plan_settings(&repo.root, &repo.artifacts_dir);

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info())
        .expect("run_plan should succeed");

    assert!(
        outcome.plan.ops.len() >= 2,
        "need multiple ops to verify IDs"
    );

    // Every op should have a non-empty ID
    for op in &outcome.plan.ops {
        assert!(
            !op.id.is_empty(),
            "op with fix_key '{}' should have a non-empty ID",
            op.rationale.fix_key
        );
    }

    // All IDs should be unique
    let ids: Vec<&str> = outcome.plan.ops.iter().map(|o| o.id.as_str()).collect();
    let unique_ids: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(
        ids.len(),
        unique_ids.len(),
        "all op IDs should be unique, got {:?}",
        ids
    );
}
