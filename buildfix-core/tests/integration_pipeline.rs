//! End-to-end integration tests for the buildfix pipeline.
//!
//! These tests exercise the full pipeline: receipt loading -> domain planning
//! -> edit engine -> artifact output, using the public API from buildfix-core.
//!
//! All tests use filesystem-backed receipt sources and real temp directories
//! to verify the pipeline works as it would in production.

use buildfix_core::adapters::FsReceiptSource;
use buildfix_core::pipeline::{run_apply, run_plan, write_plan_artifacts};
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
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8 path");
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
        std::fs::read_to_string(&path).expect("read file")
    }

    fn write_receipt(&self, sensor_name: &str, receipt_json: &str) {
        let sensor_dir = self.artifacts_dir.join(sensor_name);
        std::fs::create_dir_all(&sensor_dir).expect("create sensor dir");
        std::fs::write(sensor_dir.join("report.json"), receipt_json).expect("write receipt");
    }
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

// =============================================================================
// Test: plan resolver_v2 from receipt
// =============================================================================

#[test]
fn test_plan_resolver_v2_from_receipt() {
    let repo = TestRepo::new();

    // Workspace Cargo.toml missing resolver = "2"
    repo.write_file("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n");
    repo.write_file(
        "crates/a/Cargo.toml",
        "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    // Receipt that triggers the resolver_v2 fixer
    repo.write_receipt("builddiag", RESOLVER_V2_RECEIPT);

    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let settings = default_plan_settings(&repo.root, &repo.artifacts_dir);

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info()).unwrap();

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
    let repo = TestRepo::new();

    // Workspace missing resolver = "2", with path deps missing version
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

    // builddiag receipt for resolver_v2
    repo.write_receipt("builddiag", RESOLVER_V2_RECEIPT);

    // depguard receipt for path dep missing version
    repo.write_receipt(
        "depguard",
        &path_dep_version_receipt("crates/a/Cargo.toml", "crate-b", "../b", 7),
    );

    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let settings = default_plan_settings(&repo.root, &repo.artifacts_dir);

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info()).unwrap();

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
        fix_keys
            .iter()
            .any(|k| k.contains("workspace.resolver_v2")),
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

    // Ops should be sorted deterministically: the first op should have a
    // sort key that is lexicographically <= the second op's sort key.
    // We verify this by checking that fix_keys are in a consistent order
    // across multiple invocations (tested in test_deterministic_output).
    // Here, just verify the ops are not all targeting the same file (they
    // target different files: Cargo.toml and crates/a/Cargo.toml).
    let targets: Vec<&str> = outcome.plan.ops.iter().map(|o| o.target.path.as_str()).collect();
    assert!(
        targets.contains(&"Cargo.toml"),
        "should have Cargo.toml target"
    );
    assert!(
        targets.contains(&"crates/a/Cargo.toml"),
        "should have crates/a/Cargo.toml target"
    );

    // Summary should reflect the ops
    assert_eq!(outcome.plan.summary.ops_total, outcome.plan.ops.len() as u64);
    assert_eq!(outcome.plan.summary.ops_blocked, 0);
    assert!(outcome.plan.summary.files_touched >= 2);
}

// =============================================================================
// Test: apply produces correct files
// =============================================================================

#[test]
fn test_apply_produces_correct_files() {
    let repo = TestRepo::new();

    // Workspace Cargo.toml missing resolver = "2"
    repo.write_file("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n");
    repo.write_file(
        "crates/a/Cargo.toml",
        "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    // Receipt for resolver_v2
    repo.write_receipt("builddiag", RESOLVER_V2_RECEIPT);

    // Run plan
    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let plan_settings = default_plan_settings(&repo.root, &repo.artifacts_dir);
    let out_dir = plan_settings.out_dir.clone();

    let plan_outcome =
        run_plan(&plan_settings, &receipts_port, &NullGitPort, tool_info()).unwrap();

    // Write plan artifacts to disk so run_apply can read plan.json
    write_plan_artifacts(&plan_outcome, &out_dir, &FsWritePort).unwrap();

    // Verify plan.json was written
    assert!(
        out_dir.join("plan.json").exists(),
        "plan.json should exist after write_plan_artifacts"
    );

    // Run apply (real, non-dry-run)
    let apply_settings = default_apply_settings(&repo.root, &out_dir);

    let apply_outcome = run_apply(&apply_settings, &NullGitPort, tool_info()).unwrap();

    // Apply should have applied 1 op
    assert_eq!(
        apply_outcome.apply.summary.applied, 1,
        "expected 1 applied op, got {}",
        apply_outcome.apply.summary.applied
    );
    assert_eq!(apply_outcome.apply.summary.failed, 0);
    assert_eq!(apply_outcome.apply.summary.blocked, 0);

    // The Cargo.toml should now contain resolver = "2"
    let cargo_toml = repo.read_file("Cargo.toml");
    let normalized = normalize_line_endings(&cargo_toml);
    assert!(
        normalized.contains("resolver = \"2\""),
        "Cargo.toml should contain 'resolver = \"2\"' after apply, got:\n{}",
        normalized
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
    let repo = TestRepo::new();

    // Workspace Cargo.toml missing resolver = "2"
    repo.write_file("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n");
    repo.write_file(
        "crates/a/Cargo.toml",
        "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );

    // Receipt
    repo.write_receipt("builddiag", RESOLVER_V2_RECEIPT);

    // Run plan
    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let plan_settings = default_plan_settings(&repo.root, &repo.artifacts_dir);
    let out_dir = plan_settings.out_dir.clone();

    let plan_outcome =
        run_plan(&plan_settings, &receipts_port, &NullGitPort, tool_info()).unwrap();

    // Ensure preconditions were attached
    assert!(
        !plan_outcome.plan.preconditions.files.is_empty(),
        "plan should have file preconditions"
    );

    // Write plan artifacts to disk
    write_plan_artifacts(&plan_outcome, &out_dir, &FsWritePort).unwrap();

    // Now modify the Cargo.toml to break the SHA256 precondition
    repo.write_file(
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/a\"]\n# modified after plan\n",
    );

    // Run apply (real, non-dry-run)
    let apply_settings = default_apply_settings(&repo.root, &out_dir);

    let apply_outcome = run_apply(&apply_settings, &NullGitPort, tool_info()).unwrap();

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

    // All ops should be blocked
    assert!(
        apply_outcome.apply.summary.applied == 0,
        "no ops should have been applied"
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
        run_plan(&settings, &receipts_port, &NullGitPort, tool_info()).unwrap()
    };

    let repo1 = make_repo();
    let repo2 = make_repo();

    let outcome1 = run_pipeline(&repo1);
    let outcome2 = run_pipeline(&repo2);

    // Serialize plans to wire format for comparison (this is what gets written to disk)
    let plan_wire1 = PlanV1::try_from(&outcome1.plan).expect("convert plan1 to wire");
    let plan_wire2 = PlanV1::try_from(&outcome2.plan).expect("convert plan2 to wire");

    let plan_json1 = serde_json::to_string_pretty(&plan_wire1).expect("serialize plan1");
    let plan_json2 = serde_json::to_string_pretty(&plan_wire2).expect("serialize plan2");

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
    let repo = TestRepo::new();

    // Workspace missing resolver = "2", with path deps missing version
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

    // Both receipts
    repo.write_receipt("builddiag", RESOLVER_V2_RECEIPT);
    repo.write_receipt(
        "depguard",
        &path_dep_version_receipt("crates/a/Cargo.toml", "crate-b", "../b", 7),
    );

    // Run plan
    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let plan_settings = default_plan_settings(&repo.root, &repo.artifacts_dir);
    let out_dir = plan_settings.out_dir.clone();

    let plan_outcome =
        run_plan(&plan_settings, &receipts_port, &NullGitPort, tool_info()).unwrap();

    assert!(
        plan_outcome.plan.ops.len() >= 2,
        "expected at least 2 ops, got {}",
        plan_outcome.plan.ops.len()
    );

    // Write plan artifacts
    write_plan_artifacts(&plan_outcome, &out_dir, &FsWritePort).unwrap();

    // Run apply
    let apply_settings = default_apply_settings(&repo.root, &out_dir);
    let apply_outcome = run_apply(&apply_settings, &NullGitPort, tool_info()).unwrap();

    // All ops should have been applied
    assert_eq!(apply_outcome.apply.summary.failed, 0);
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

    let outcome = run_plan(&settings, &receipts_port, &NullGitPort, tool_info()).unwrap();

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
    let repo = TestRepo::new();

    repo.write_file("Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n");
    repo.write_file(
        "crates/a/Cargo.toml",
        "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    repo.write_receipt("builddiag", RESOLVER_V2_RECEIPT);

    let receipts_port = FsReceiptSource::new(repo.artifacts_dir.clone());
    let plan_settings = default_plan_settings(&repo.root, &repo.artifacts_dir);
    let out_dir = plan_settings.out_dir.clone();

    let plan_outcome =
        run_plan(&plan_settings, &receipts_port, &NullGitPort, tool_info()).unwrap();

    write_plan_artifacts(&plan_outcome, &out_dir, &FsWritePort).unwrap();

    // Read and validate plan.json
    let plan_json = std::fs::read_to_string(out_dir.join("plan.json")).expect("read plan.json");
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
    let report_json =
        std::fs::read_to_string(out_dir.join("report.json")).expect("read report.json");
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
    let patch = std::fs::read_to_string(out_dir.join("patch.diff")).expect("read patch.diff");
    assert!(!patch.is_empty(), "patch.diff should not be empty");
}
