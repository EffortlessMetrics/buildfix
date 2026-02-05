//! Golden fixture tests for buildfix.
//!
//! These tests verify that the planner produces deterministic, expected output
//! for known input scenarios. Each fixture contains:
//!
//! - `repo/` - The repository state (Cargo.toml files)
//! - `receipts/` - Sensor receipts that trigger fixes
//! - `expected/` - Expected output files (plan.json, plan.md, patch.diff)

use buildfix_domain::{FsRepoView, PlanContext, Planner, PlannerConfig};
use buildfix_render::{render_apply_md, render_plan_md};
use buildfix_types::receipt::ToolInfo;
use buildfix_types::wire::{ApplyV1, PlanV1};
use camino::Utf8PathBuf;
use fs_err as fs;
use pretty_assertions::assert_eq;
use std::path::Path;
use tempfile::TempDir;

/// Strips dynamic fields from a plan JSON for comparison.
fn normalize_plan_json(json: &str) -> serde_json::Value {
    let mut v: serde_json::Value = serde_json::from_str(json).expect("valid JSON");

    if let Some(obj) = v.as_object_mut() {
        // Normalize repo root
        if let Some(repo) = obj.get_mut("repo").and_then(|r| r.as_object_mut()) {
            repo.insert("root".to_string(), serde_json::json!("<REPO_ROOT>"));
            repo.remove("head_sha");
            repo.remove("dirty");
        }

        // Normalize inputs paths
        if let Some(inputs) = obj.get_mut("inputs").and_then(|i| i.as_array_mut()) {
            for input in inputs {
                if let Some(i) = input.as_object_mut() {
                    if let Some(path) = i.get("path").and_then(|p| p.as_str()) {
                        let path_normalized = path.replace('\\', "/");
                        if let Some(idx) = path_normalized.rfind("/artifacts/") {
                            let after = &path_normalized[idx + "/artifacts/".len()..];
                            i.insert(
                                "path".to_string(),
                                serde_json::json!(format!("<ARTIFACTS>/{}", after)),
                            );
                        }
                    }
                }
            }
        }

        // Normalize precondition hashes
        if let Some(pre) = obj.get_mut("preconditions").and_then(|p| p.as_object_mut()) {
            if let Some(files) = pre.get_mut("files").and_then(|f| f.as_array_mut()) {
                for file in files {
                    if let Some(f) = file.as_object_mut() {
                        f.insert("sha256".to_string(), serde_json::json!("<SHA256>"));
                    }
                }
            }
            pre.remove("head_sha");
            pre.remove("dirty");
        }
    }

    v
}

/// Strips dynamic fields from an apply JSON for comparison.
fn normalize_apply_json(json: &str) -> serde_json::Value {
    let mut v: serde_json::Value = serde_json::from_str(json).expect("valid JSON");

    if let Some(obj) = v.as_object_mut() {
        if let Some(repo) = obj.get_mut("repo").and_then(|r| r.as_object_mut()) {
            repo.insert("root".to_string(), serde_json::json!("<REPO_ROOT>"));
            repo.remove("head_sha_before");
            repo.remove("head_sha_after");
            repo.remove("dirty_before");
            repo.remove("dirty_after");
        }

        if let Some(plan_ref) = obj.get_mut("plan_ref").and_then(|r| r.as_object_mut()) {
            plan_ref.insert("path".to_string(), serde_json::json!("<PLAN_PATH>"));
            plan_ref.remove("sha256");
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
        config: PlannerConfig::default(),
    };
    let repo = FsRepoView::new(repo_root.clone());
    let tool = ToolInfo {
        name: "buildfix".to_string(),
        version: Some("test".to_string()),
        repo: None,
        commit: None,
    };

    let mut plan = planner
        .plan(&ctx, &repo, &receipts, tool.clone())
        .expect("generate plan");

    // Attach preconditions (but we'll strip sha256 for comparison)
    let attach_opts = buildfix_edit::AttachPreconditionsOptions {
        include_git_head: false,
    };
    buildfix_edit::attach_preconditions(&repo_root, &mut plan, &attach_opts)
        .expect("attach preconditions");

    // Generate patch preview
    let preview_opts = buildfix_edit::ApplyOptions {
        dry_run: true,
        allow_guarded: true,
        allow_unsafe: true,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".buildfix.bak".to_string(),
        params: std::collections::HashMap::new(),
    };
    let patch = buildfix_edit::preview_patch(&repo_root, &plan, &preview_opts)
        .expect("preview patch");

    // Serialize and normalize
    let plan_wire = PlanV1::try_from(&plan).expect("convert plan to wire");
    let plan_json = serde_json::to_string_pretty(&plan_wire).expect("serialize plan");
    let normalized = normalize_plan_json(&plan_json);

    let expected_dir = fixture_path.join("expected");
    let expected_plan_path = expected_dir.join("plan.json");
    let expected_md_path = expected_dir.join("plan.md");
    let expected_patch_path = expected_dir.join("patch.diff");
    let expected_apply_path = expected_dir.join("apply.json");
    let expected_apply_md_path = expected_dir.join("apply.md");

    let bless = std::env::var("BUILDFIX_BLESS").ok().as_deref() == Some("1");

    if bless {
        fs::create_dir_all(&expected_dir).expect("create expected dir");
        let output = serde_json::to_string_pretty(&normalized).expect("serialize normalized");
        fs::write(&expected_plan_path, output).expect("write expected plan");
    } else if expected_plan_path.exists() {
        let expected_json = fs::read_to_string(&expected_plan_path).expect("read expected plan");
        let expected_normalized = normalize_plan_json(&expected_json);
        assert_eq!(
            normalized, expected_normalized,
            "Plan mismatch for fixture '{}'",
            fixture_name
        );
    } else {
        panic!("missing expected plan for fixture '{}'", fixture_name);
    }

    let plan_md = render_plan_md(&plan);
    if bless {
        fs::create_dir_all(&expected_dir).expect("create expected dir");
        fs::write(&expected_md_path, plan_md).expect("write expected plan.md");
    } else if expected_md_path.exists() {
        let expected_md = fs::read_to_string(&expected_md_path).expect("read expected plan.md");
        assert_eq!(
            plan_md, expected_md,
            "plan.md mismatch for fixture '{}'",
            fixture_name
        );
    } else {
        panic!("missing expected plan.md for fixture '{}'", fixture_name);
    }

    if bless {
        fs::create_dir_all(&expected_dir).expect("create expected dir");
        fs::write(&expected_patch_path, patch).expect("write expected patch.diff");
    } else if expected_patch_path.exists() {
        let expected_patch =
            fs::read_to_string(&expected_patch_path).expect("read expected patch.diff");
        assert_eq!(
            patch, expected_patch,
            "patch.diff mismatch for fixture '{}'",
            fixture_name
        );
    } else {
        panic!("missing expected patch.diff for fixture '{}'", fixture_name);
    }

    // Apply expectations (only when expected files exist or when blessing).
    if expected_apply_path.exists() || bless {
        let apply_opts = buildfix_edit::ApplyOptions {
            dry_run: false,
            allow_guarded: true,
            allow_unsafe: true,
            backup_enabled: false,
            backup_dir: None,
            backup_suffix: ".buildfix.bak".to_string(),
            params: std::collections::HashMap::new(),
        };

        let (apply, _patch) = buildfix_edit::apply_plan(&repo_root, &plan, tool, &apply_opts)
            .expect("apply plan");

        let apply_wire = ApplyV1::try_from(&apply).expect("convert apply to wire");
        let apply_json = serde_json::to_string_pretty(&apply_wire).expect("serialize apply");
        let normalized_apply = normalize_apply_json(&apply_json);

        if bless {
            fs::create_dir_all(&expected_dir).expect("create expected dir");
            let output =
                serde_json::to_string_pretty(&normalized_apply).expect("serialize normalized");
            fs::write(&expected_apply_path, output).expect("write expected apply");
        } else if expected_apply_path.exists() {
            let expected_json =
                fs::read_to_string(&expected_apply_path).expect("read expected apply");
            let expected_normalized = normalize_apply_json(&expected_json);
            assert_eq!(
                normalized_apply, expected_normalized,
                "apply.json mismatch for fixture '{}'",
                fixture_name
            );
        }

        let apply_md = render_apply_md(&apply);
        if bless {
            fs::create_dir_all(&expected_dir).expect("create expected dir");
            fs::write(&expected_apply_md_path, apply_md).expect("write expected apply.md");
        } else if expected_apply_md_path.exists() {
            let expected_md =
                fs::read_to_string(&expected_apply_md_path).expect("read expected apply.md");
            assert_eq!(
                apply_md, expected_md,
                "apply.md mismatch for fixture '{}'",
                fixture_name
            );
        }
    }
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
