//! Precondition validation tests.

use buildfix_edit::{apply_plan, attach_preconditions, ApplyOptions, AttachPreconditionsOptions};
use buildfix_types::apply::ApplyStatus;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{
    BuildfixPlan, FilePrecondition, PlanOp, PlanPolicy, Rationale, RepoInfo,
};
use buildfix_types::receipt::ToolInfo;
use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

fn create_temp_repo() -> TempDir {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();

    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/a"]
"#,
    )
    .unwrap();

    td
}

fn sha256_hex(contents: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(contents.as_bytes());
    hex::encode(hasher.finalize())
}

fn tool_info() -> ToolInfo {
    ToolInfo {
        name: "buildfix".to_string(),
        version: Some("0.0.0".to_string()),
        repo: None,
        commit: None,
    }
}

fn repo_info() -> RepoInfo {
    RepoInfo {
        root: ".".to_string(),
        head_sha: None,
        dirty: None,
    }
}

fn minimal_plan_with_preconditions(file_path: &str, expected_sha: &str) -> BuildfixPlan {
    let mut plan = BuildfixPlan::new(tool_info(), repo_info(), PlanPolicy::default());
    plan.preconditions.files.push(FilePrecondition {
        path: file_path.to_string(),
        sha256: expected_sha.to_string(),
    });
    plan.ops.push(PlanOp {
        id: "test-op".to_string(),
        safety: SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        target: OpTarget {
            path: file_path.to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        rationale: Rationale {
            fix_key: "test/test/test".to_string(),
            description: Some("test".to_string()),
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    });
    plan
}

#[test]
fn test_matching_sha_allows_apply() {
    let temp = create_temp_repo();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();

    let contents = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let expected_sha = sha256_hex(&contents);

    let plan = minimal_plan_with_preconditions("Cargo.toml", &expected_sha);

    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).unwrap();

    assert!(apply.preconditions.verified);
    assert!(apply.preconditions.mismatches.is_empty());
}

#[test]
fn test_sha_mismatch_blocks_apply() {
    let temp = create_temp_repo();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();

    // Use wrong hash
    let plan = minimal_plan_with_preconditions(
        "Cargo.toml",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );

    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).unwrap();

    assert!(!apply.preconditions.verified);
    assert_eq!(apply.preconditions.mismatches.len(), 1);
    assert_eq!(apply.preconditions.mismatches[0].path, "Cargo.toml");

    // All ops should be blocked due to precondition mismatch
    for result in &apply.results {
        assert_eq!(result.status, ApplyStatus::Blocked);
        assert!(result
            .blocked_reason
            .as_ref()
            .map(|r| r.contains("precondition"))
            .unwrap_or(false));
    }
}

#[test]
fn test_file_modified_after_plan_blocks_apply() {
    let temp = create_temp_repo();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();

    let initial_contents = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let expected_sha = sha256_hex(&initial_contents);

    let plan = minimal_plan_with_preconditions("Cargo.toml", &expected_sha);

    // Modify the file AFTER computing preconditions
    fs::write(
        temp.path().join("Cargo.toml"),
        format!("{}\n# modified\n", initial_contents),
    )
    .unwrap();

    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).unwrap();

    assert!(!apply.preconditions.verified);
    assert!(!apply.preconditions.mismatches.is_empty());
}

#[test]
fn test_attach_preconditions_computes_correct_sha() {
    let temp = create_temp_repo();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();

    let contents = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let expected_sha = sha256_hex(&contents);

    let mut plan = BuildfixPlan::new(tool_info(), repo_info(), PlanPolicy::default());
    plan.ops.push(PlanOp {
        id: "test-op".to_string(),
        safety: SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        target: OpTarget {
            path: "Cargo.toml".to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        rationale: Rationale {
            fix_key: "test/test/test".to_string(),
            description: Some("test".to_string()),
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    });

    attach_preconditions(&root, &mut plan, &AttachPreconditionsOptions::default()).unwrap();

    assert_eq!(plan.preconditions.files.len(), 1);
    assert_eq!(plan.preconditions.files[0].path, "Cargo.toml");
    assert_eq!(plan.preconditions.files[0].sha256, expected_sha);
}

#[test]
fn test_dry_run_with_valid_preconditions_shows_skipped() {
    let temp = create_temp_repo();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();

    let contents = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let expected_sha = sha256_hex(&contents);

    // Use correct hash - dry run should show skipped (not applied because dry-run)
    let plan = minimal_plan_with_preconditions("Cargo.toml", &expected_sha);

    let opts = ApplyOptions {
        dry_run: true, // Dry run - should show skipped, not applied
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).unwrap();

    // Dry run with valid preconditions should show skipped
    for result in &apply.results {
        assert_eq!(result.status, ApplyStatus::Skipped);
    }
}

#[test]
fn test_multiple_files_preconditions() {
    let temp = create_temp_repo();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();

    // Create a second file
    fs::create_dir_all(temp.path().join("crates").join("a")).unwrap();
    fs::write(
        temp.path().join("crates").join("a").join("Cargo.toml"),
        r#"
[package]
name = "a"
version = "0.1.0"
"#,
    )
    .unwrap();

    let contents1 = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let sha1 = sha256_hex(&contents1);

    let contents2 =
        fs::read_to_string(temp.path().join("crates").join("a").join("Cargo.toml")).unwrap();
    let sha2 = sha256_hex(&contents2);

    let mut plan = BuildfixPlan::new(tool_info(), repo_info(), PlanPolicy::default());
    plan.preconditions.files.push(FilePrecondition {
        path: "Cargo.toml".to_string(),
        sha256: sha1,
    });
    plan.preconditions.files.push(FilePrecondition {
        path: "crates/a/Cargo.toml".to_string(),
        sha256: sha2,
    });
    plan.ops.push(PlanOp {
        id: "op1".to_string(),
        safety: SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        target: OpTarget {
            path: "Cargo.toml".to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        rationale: Rationale {
            fix_key: "test/test/test".to_string(),
            description: Some("test".to_string()),
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    });
    plan.ops.push(PlanOp {
        id: "op2".to_string(),
        safety: SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        target: OpTarget {
            path: "crates/a/Cargo.toml".to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: "set_package_rust_version".to_string(),
            args: Some(serde_json::json!({"rust_version": "1.70"})),
        },
        rationale: Rationale {
            fix_key: "test/test/test".to_string(),
            description: Some("test".to_string()),
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    });

    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).unwrap();

    assert!(apply.preconditions.verified);
    assert!(apply.preconditions.mismatches.is_empty());
}

#[test]
fn test_one_file_mismatch_blocks_all_ops() {
    let temp = create_temp_repo();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();

    // Create a second file
    fs::create_dir_all(temp.path().join("crates").join("a")).unwrap();
    fs::write(
        temp.path().join("crates").join("a").join("Cargo.toml"),
        r#"
[package]
name = "a"
version = "0.1.0"
"#,
    )
    .unwrap();

    let contents1 = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let sha1 = sha256_hex(&contents1);

    // Wrong SHA for second file
    let wrong_sha2 = "0000000000000000000000000000000000000000000000000000000000000000";

    let mut plan = BuildfixPlan::new(tool_info(), repo_info(), PlanPolicy::default());
    plan.preconditions.files.push(FilePrecondition {
        path: "Cargo.toml".to_string(),
        sha256: sha1,
    });
    plan.preconditions.files.push(FilePrecondition {
        path: "crates/a/Cargo.toml".to_string(),
        sha256: wrong_sha2.to_string(),
    });
    plan.ops.push(PlanOp {
        id: "op1".to_string(),
        safety: SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        target: OpTarget {
            path: "Cargo.toml".to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        rationale: Rationale {
            fix_key: "test/test/test".to_string(),
            description: Some("test".to_string()),
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    });
    plan.ops.push(PlanOp {
        id: "op2".to_string(),
        safety: SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        target: OpTarget {
            path: "crates/a/Cargo.toml".to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: "set_package_rust_version".to_string(),
            args: Some(serde_json::json!({"rust_version": "1.70"})),
        },
        rationale: Rationale {
            fix_key: "test/test/test".to_string(),
            description: Some("test".to_string()),
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    });

    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).unwrap();

    // One mismatch should block ALL ops
    assert!(!apply.preconditions.verified);
    assert_eq!(apply.preconditions.mismatches.len(), 1);

    for result in &apply.results {
        assert_eq!(result.status, ApplyStatus::Blocked);
    }
}

#[test]
fn test_empty_preconditions_allows_apply() {
    let temp = create_temp_repo();
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();

    // Plan with no file preconditions (e.g., generated with --no-clean-hashes)
    let mut plan = BuildfixPlan::new(tool_info(), repo_info(), PlanPolicy::default());
    plan.ops.push(PlanOp {
        id: "test-op".to_string(),
        safety: SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        target: OpTarget {
            path: "Cargo.toml".to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        rationale: Rationale {
            fix_key: "test/test/test".to_string(),
            description: Some("test".to_string()),
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    });

    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).unwrap();

    // Empty preconditions should be verified (nothing to check)
    assert!(apply.preconditions.verified);
}
