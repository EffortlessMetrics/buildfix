//! Golden fixture tests for buildfix.
//!
//! These tests verify that the planner produces deterministic, expected output
//! for known input scenarios. Each fixture contains:
//!
//! - `repo/` - The repository state (Cargo.toml files)
//! - `receipts/` - Sensor receipts that trigger fixes
//! - `expected/` - Expected output files (plan.json, plan.md, patch.diff)

use buildfix_domain::{FsRepoView, PlanContext, Planner, PlannerConfig};
use buildfix_types::plan::PolicyCaps;
use buildfix_types::receipt::ToolInfo;
use camino::Utf8PathBuf;
use fs_err as fs;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;

/// Strips dynamic fields from a plan JSON for comparison.
///
/// Removes: plan_id, run.started_at, run.ended_at, git_head_sha, all sha256 hashes
fn normalize_plan_json(json: &str) -> serde_json::Value {
    let mut v: serde_json::Value = serde_json::from_str(json).expect("valid JSON");

    // Remove dynamic fields at top level
    if let Some(obj) = v.as_object_mut() {
        obj.remove("plan_id");

        // Normalize run timestamps
        if let Some(run) = obj.get_mut("run").and_then(|r| r.as_object_mut()) {
            run.remove("started_at");
            run.remove("ended_at");
            run.remove("git_head_sha");
        }

        // Normalize inputs paths (make them generic)
        if let Some(inputs) = obj.get_mut("inputs").and_then(|i| i.as_object_mut()) {
            inputs.insert("repo_root".to_string(), serde_json::json!("<REPO_ROOT>"));
            inputs.insert(
                "artifacts_dir".to_string(),
                serde_json::json!("<ARTIFACTS_DIR>"),
            );
        }

        // Normalize receipts paths
        if let Some(receipts) = obj.get_mut("receipts").and_then(|r| r.as_array_mut()) {
            for receipt in receipts {
                if let Some(r) = receipt.as_object_mut() {
                    if let Some(path) = r.get("report_path").and_then(|p| p.as_str()) {
                        // Keep only the sensor + report.json portion
                        // Input: /tmp/xxx/artifacts/builddiag/report.json
                        // Output: <ARTIFACTS>/<sensor>/report.json
                        let path_normalized = path.replace('\\', "/");
                        if let Some(idx) = path_normalized.rfind("/artifacts/") {
                            let after_artifacts = &path_normalized[idx + "/artifacts/".len()..];
                            r.insert(
                                "report_path".to_string(),
                                serde_json::json!(format!("<ARTIFACTS>/{}", after_artifacts)),
                            );
                        }
                    }
                }
            }
        }

        // Remove preconditions with sha256 hashes from fixes
        if let Some(fixes) = obj.get_mut("fixes").and_then(|f| f.as_array_mut()) {
            for fix in fixes {
                if let Some(f) = fix.as_object_mut() {
                    // Remove the dynamic fix instance id
                    f.remove("id");

                    // Filter out FileSha256 preconditions (keep FileExists)
                    if let Some(preconds) =
                        f.get_mut("preconditions").and_then(|p| p.as_array_mut())
                    {
                        preconds.retain(|p| {
                            p.get("type")
                                .and_then(|t| t.as_str())
                                .map(|t| t != "file_sha256" && t != "git_head_sha")
                                .unwrap_or(true)
                        });
                    }
                }
            }
        }
    }

    v
}

/// Runs a fixture test, comparing generated output against expected output.
fn run_fixture_test(fixture_name: &str) {
    // Fixtures are at workspace root: ../tests/fixtures relative to buildfix-domain
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().expect("workspace root");
    let fixtures_dir = workspace_root.join("tests").join("fixtures");
    let fixture_path = fixtures_dir.join(fixture_name);

    assert!(
        fixture_path.exists(),
        "Fixture directory does not exist: {}",
        fixture_path.display()
    );

    // Copy repo to tempdir for isolation
    let temp_dir = TempDir::new().expect("create temp dir");
    let temp_repo = temp_dir.path();

    // Copy repo files
    let repo_src = fixture_path.join("repo");
    copy_dir_all(&repo_src, temp_repo).expect("copy repo");

    // Copy receipts to artifacts directory
    let artifacts_dir = temp_dir.path().join("artifacts");
    let receipts_src = fixture_path.join("receipts");
    if receipts_src.exists() {
        copy_dir_all(&receipts_src, &artifacts_dir).expect("copy receipts");
    }

    // Run the planner
    let repo_root = Utf8PathBuf::from_path_buf(temp_repo.to_path_buf()).expect("utf8 path");
    let artifacts_dir_utf8 = Utf8PathBuf::from_path_buf(artifacts_dir.clone()).expect("utf8 path");

    let receipts = buildfix_receipts::load_receipts(&artifacts_dir_utf8).expect("load receipts");

    let planner = Planner::new();
    let ctx = PlanContext {
        repo_root: repo_root.clone(),
        artifacts_dir: artifacts_dir_utf8,
        config: PlannerConfig {
            allow: vec![],
            deny: vec![],
            require_clean_hashes: true,
            caps: PolicyCaps::default(),
        },
    };
    let repo = FsRepoView::new(repo_root.clone());
    let tool = ToolInfo {
        name: "buildfix".to_string(),
        version: Some("test".to_string()),
        repo: None,
        commit: None,
    };

    let mut plan = planner
        .plan(&ctx, &repo, &receipts, tool)
        .expect("generate plan");

    // Attach preconditions (but we'll strip sha256 for comparison)
    let attach_opts = buildfix_edit::AttachPreconditionsOptions {
        include_git_head: false,
    };
    buildfix_edit::attach_preconditions(&repo_root, &mut plan, &attach_opts)
        .expect("attach preconditions");

    // Serialize and normalize
    let plan_json = serde_json::to_string_pretty(&plan).expect("serialize plan");
    let normalized = normalize_plan_json(&plan_json);

    // Check if expected files exist
    let expected_dir = fixture_path.join("expected");
    let expected_plan_path = expected_dir.join("plan.json");

    if expected_plan_path.exists() {
        // Compare against expected
        let expected_json = fs::read_to_string(&expected_plan_path).expect("read expected plan");
        let expected_normalized = normalize_plan_json(&expected_json);

        assert_eq!(
            normalized, expected_normalized,
            "Plan mismatch for fixture '{}'",
            fixture_name
        );
    } else {
        // Write the expected file for first run (bootstrap mode)
        fs::create_dir_all(&expected_dir).expect("create expected dir");
        let output = serde_json::to_string_pretty(&normalized).expect("serialize normalized");
        fs::write(&expected_plan_path, output).expect("write expected plan");
        println!(
            "Created expected plan for '{}' at {}",
            fixture_name,
            expected_plan_path.display()
        );
    }

    // Also verify plan summary matches fix counts
    let safe_count = plan
        .fixes
        .iter()
        .filter(|f| f.safety == buildfix_types::ops::SafetyClass::Safe)
        .count() as u64;
    let guarded_count = plan
        .fixes
        .iter()
        .filter(|f| f.safety == buildfix_types::ops::SafetyClass::Guarded)
        .count() as u64;

    assert_eq!(plan.summary.safe, safe_count, "Summary safe count mismatch");
    assert_eq!(
        plan.summary.guarded, guarded_count,
        "Summary guarded count mismatch"
    );
    assert_eq!(
        plan.summary.fixes_total,
        plan.fixes.len() as u64,
        "Summary total count mismatch"
    );
}

/// Recursively copy a directory.
fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

#[test]
fn golden_resolver_v2() {
    run_fixture_test("resolver_v2");
}

#[test]
fn golden_path_dep_version() {
    run_fixture_test("path_dep_version");
}

#[test]
fn golden_workspace_inheritance() {
    run_fixture_test("workspace_inheritance");
}

#[test]
fn golden_msrv_normalize() {
    run_fixture_test("msrv_normalize");
}

#[test]
fn golden_multi_fix() {
    run_fixture_test("multi_fix");
}
