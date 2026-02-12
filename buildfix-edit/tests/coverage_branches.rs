use buildfix_edit::{
    ApplyOptions, AttachPreconditionsOptions, apply_op_to_content, apply_plan,
    attach_preconditions, check_policy_block, get_head_sha, is_working_tree_dirty, preview_patch,
};
use buildfix_types::apply::{
    ApplyPreconditions, ApplyRepoInfo, ApplyResult, ApplyStatus, ApplySummary, BuildfixApply,
    PlanRef,
};
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{BuildfixPlan, PlanOp, PlanPolicy, Rationale, RepoInfo};
use buildfix_types::receipt::ToolInfo;
use camino::{Utf8Path, Utf8PathBuf};
use fs_err as fs;
use std::collections::{BTreeMap, HashMap};
use std::process::Command;
use tempfile::TempDir;

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

fn base_plan() -> BuildfixPlan {
    BuildfixPlan::new(tool_info(), repo_info(), PlanPolicy::default())
}

fn make_op(
    id: &str,
    path: &str,
    safety: SafetyClass,
    blocked: bool,
    kind: OpKind,
    params_required: Vec<String>,
) -> PlanOp {
    PlanOp {
        id: id.to_string(),
        safety,
        blocked,
        blocked_reason: None,
        blocked_reason_token: None,
        target: OpTarget {
            path: path.to_string(),
        },
        kind,
        rationale: Rationale {
            fix_key: "test/test/test".to_string(),
            description: Some("test".to_string()),
            findings: vec![],
        },
        params_required,
        preview: None,
    }
}

fn run_git(root: &Utf8Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .expect("run git");
    assert!(status.success(), "git {:?} failed", args);
}

#[test]
fn attach_preconditions_includes_git_head_and_dirty() {
    let temp = TempDir::new().expect("temp dir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");

    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write");

    run_git(&root, &["init"]);
    run_git(&root, &["config", "user.email", "test@example.com"]);
    run_git(&root, &["config", "user.name", "Test User"]);
    run_git(&root, &["add", "."]);
    run_git(&root, &["commit", "-m", "init"]);

    let mut plan = base_plan();
    plan.ops.push(make_op(
        "op1",
        "Cargo.toml",
        SafetyClass::Safe,
        false,
        OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        vec![],
    ));

    attach_preconditions(
        &root,
        &mut plan,
        &AttachPreconditionsOptions {
            include_git_head: true,
        },
    )
    .expect("attach");

    let head = get_head_sha(&root).expect("head sha");
    assert_eq!(plan.preconditions.head_sha, Some(head));
    assert_eq!(plan.preconditions.dirty, Some(false));
    assert!(!is_working_tree_dirty(&root).expect("dirty"));

    fs::write(root.join("Cargo.toml"), "[workspace]\n# dirty\n").expect("write");
    attach_preconditions(
        &root,
        &mut plan,
        &AttachPreconditionsOptions {
            include_git_head: true,
        },
    )
    .expect("attach dirty");
    assert_eq!(plan.preconditions.dirty, Some(true));
}

#[test]
fn preview_patch_emits_diff() {
    let temp = TempDir::new().expect("temp dir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write");

    let mut plan = base_plan();
    plan.ops.push(make_op(
        "op1",
        "Cargo.toml",
        SafetyClass::Safe,
        false,
        OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        vec![],
    ));

    let opts = ApplyOptions {
        dry_run: true,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let patch = preview_patch(&root, &plan, &opts).expect("preview");
    assert!(patch.contains("diff --git"));
    assert!(patch.contains("workspace"));
}

#[test]
fn apply_plan_writes_backups() {
    let temp = TempDir::new().expect("temp dir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");

    fs::create_dir_all(root.join("crates").join("a")).expect("mkdir");
    fs::write(
        root.join("crates").join("a").join("Cargo.toml"),
        "[package]\nname = \"a\"\n",
    )
    .expect("write");

    let mut plan = base_plan();
    plan.ops.push(make_op(
        "op1",
        "crates/a/Cargo.toml",
        SafetyClass::Safe,
        false,
        OpKind::TomlTransform {
            rule_id: "set_package_rust_version".to_string(),
            args: Some(serde_json::json!({"rust_version": "1.70"})),
        },
        vec![],
    ));

    let backup_dir = Utf8PathBuf::from_path_buf(temp.path().join("backups")).expect("utf8");
    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: true,
        backup_dir: Some(backup_dir.clone()),
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).expect("apply");
    let result = apply.results.iter().find(|r| r.op_id == "op1").unwrap();
    assert_eq!(result.status, ApplyStatus::Applied);
    let file = result
        .files
        .iter()
        .find(|f| f.path == "crates/a/Cargo.toml")
        .unwrap();
    let backup_path = Utf8Path::new(file.backup_path.as_ref().expect("backup path"));
    assert!(backup_path.exists());
}

#[test]
fn apply_plan_records_block_reasons() {
    let temp = TempDir::new().expect("temp dir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write");

    let mut plan = base_plan();
    plan.ops.push(make_op(
        "blocked",
        "Cargo.toml",
        SafetyClass::Safe,
        true,
        OpKind::TomlSet {
            toml_path: vec!["package".to_string(), "name".to_string()],
            value: serde_json::Value::String("demo".to_string()),
        },
        vec![],
    ));
    plan.ops.push(make_op(
        "missing_params",
        "Cargo.toml",
        SafetyClass::Safe,
        false,
        OpKind::TomlTransform {
            rule_id: "set_package_edition".to_string(),
            args: None,
        },
        vec!["edition".to_string()],
    ));
    plan.ops.push(make_op(
        "safety_blocked",
        "Cargo.toml",
        SafetyClass::Guarded,
        false,
        OpKind::TomlSet {
            toml_path: vec!["package".to_string(), "name".to_string()],
            value: serde_json::Value::String("demo".to_string()),
        },
        vec![],
    ));
    plan.ops.push(make_op(
        "blocked_with_params",
        "Cargo.toml",
        SafetyClass::Safe,
        true,
        OpKind::TomlTransform {
            rule_id: "set_package_rust_version".to_string(),
            args: None,
        },
        vec!["rust_version".to_string()],
    ));

    let mut params = HashMap::new();
    params.insert("rust_version".to_string(), "1.70".to_string());

    let opts = ApplyOptions {
        dry_run: true,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params,
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).expect("apply");
    let blocked = apply.results.iter().find(|r| r.op_id == "blocked").unwrap();
    assert_eq!(blocked.status, ApplyStatus::Blocked);
    assert_eq!(blocked.blocked_reason.as_deref(), Some("blocked"));

    let missing = apply
        .results
        .iter()
        .find(|r| r.op_id == "missing_params")
        .unwrap();
    assert_eq!(missing.status, ApplyStatus::Blocked);
    assert!(
        missing
            .blocked_reason
            .as_ref()
            .unwrap()
            .contains("missing params")
    );

    let safety = apply
        .results
        .iter()
        .find(|r| r.op_id == "safety_blocked")
        .unwrap();
    assert_eq!(safety.status, ApplyStatus::Blocked);
    assert_eq!(safety.blocked_reason.as_deref(), Some("safety gate"));
    assert_eq!(safety.message.as_deref(), Some("safety class not allowed"));

    let allowed = apply
        .results
        .iter()
        .find(|r| r.op_id == "blocked_with_params")
        .unwrap();
    assert_eq!(allowed.status, ApplyStatus::Skipped);
}

#[test]
fn apply_op_to_content_handles_missing_params_and_unknown_rules() {
    let contents = "[package]\nname = \"demo\"\n";

    let missing = OpKind::TomlTransform {
        rule_id: "set_package_rust_version".to_string(),
        args: None,
    };
    let err = apply_op_to_content(contents, &missing).expect_err("missing param");
    assert!(err.to_string().contains("missing rust_version"));

    let unknown = OpKind::TomlTransform {
        rule_id: "unknown_rule".to_string(),
        args: None,
    };
    let out = apply_op_to_content(contents, &unknown).expect("no-op");
    assert!(out.contains("name = \"demo\""));
}

#[test]
fn apply_op_to_content_updates_target_dependency() {
    let contents = r#"
[target."cfg(windows)".dependencies]
foo = { path = "../foo" }
"#;

    let kind = OpKind::TomlTransform {
        rule_id: "ensure_path_dep_has_version".to_string(),
        args: Some(serde_json::json!({
            "toml_path": ["target", "cfg(windows)", "dependencies", "foo"],
            "dep_path": "../foo",
            "version": "1.2.3"
        })),
    };

    let out = apply_op_to_content(contents, &kind).expect("apply");
    assert!(out.contains("version = \"1.2.3\""));
}

#[test]
fn check_policy_block_classifies_cases() {
    let mut apply = BuildfixApply::new(
        tool_info(),
        ApplyRepoInfo {
            root: ".".to_string(),
            head_sha_before: None,
            head_sha_after: None,
            dirty_before: None,
            dirty_after: None,
        },
        PlanRef {
            path: "artifacts/buildfix/plan.json".to_string(),
            sha256: None,
        },
    );
    apply.preconditions.verified = true;

    assert!(check_policy_block(&apply, true).is_none());

    let mut preconditions = apply.clone();
    preconditions.preconditions = ApplyPreconditions {
        verified: false,
        mismatches: vec![],
    };
    let err = check_policy_block(&preconditions, false).expect("policy block");
    assert!(format!("{:?}", err).contains("PreconditionMismatch"));

    let mut safety_block = apply.clone();
    safety_block.results.push(ApplyResult {
        op_id: "op1".to_string(),
        status: ApplyStatus::Blocked,
        message: None,
        blocked_reason: Some("safety gate".to_string()),
        blocked_reason_token: None,
        files: vec![],
    });
    let err = check_policy_block(&safety_block, false).expect("policy block");
    assert!(format!("{:?}", err).contains("SafetyGateDenial"));

    let mut policy_block = apply.clone();
    policy_block.results.push(ApplyResult {
        op_id: "op2".to_string(),
        status: ApplyStatus::Blocked,
        message: None,
        blocked_reason: Some("policy".to_string()),
        blocked_reason_token: None,
        files: vec![],
    });
    let err = check_policy_block(&policy_block, false).expect("policy block");
    assert!(format!("{:?}", err).contains("PolicyDenial"));

    let mut failed = apply.clone();
    failed.summary = ApplySummary {
        failed: 1,
        ..ApplySummary::default()
    };
    let err = check_policy_block(&failed, false).expect("policy block");
    assert!(format!("{:?}", err).contains("PreconditionMismatch"));
}

#[test]
fn apply_op_to_content_set_remove_and_json_values() {
    let contents = "[package]\nname = \"demo\"\n";

    let set_bool = OpKind::TomlSet {
        toml_path: vec!["package".to_string(), "publish".to_string()],
        value: serde_json::Value::Bool(false),
    };
    let out = apply_op_to_content(contents, &set_bool).expect("set bool");
    assert!(out.contains("publish = false"));

    let set_int = OpKind::TomlSet {
        toml_path: vec![
            "package".to_string(),
            "metadata".to_string(),
            "count".to_string(),
        ],
        value: serde_json::Value::Number(1.into()),
    };
    let out = apply_op_to_content(&out, &set_int).expect("set int");
    assert!(out.contains("count = 1"));

    let set_float = OpKind::TomlSet {
        toml_path: vec![
            "package".to_string(),
            "metadata".to_string(),
            "ratio".to_string(),
        ],
        value: serde_json::Value::Number(serde_json::Number::from_f64(1.5).unwrap()),
    };
    let out = apply_op_to_content(&out, &set_float).expect("set float");
    assert!(out.contains("ratio = 1.5"));

    let set_array = OpKind::TomlSet {
        toml_path: vec![
            "package".to_string(),
            "metadata".to_string(),
            "items".to_string(),
        ],
        value: serde_json::json!(["a", true, 1, 1.5, {"ignored": true}]),
    };
    let out = apply_op_to_content(&out, &set_array).expect("set array");
    assert!(out.contains("items ="));

    let remove = OpKind::TomlRemove {
        toml_path: vec!["package".to_string(), "name".to_string()],
    };
    let out = apply_op_to_content(&out, &remove).expect("remove");
    assert!(!out.contains("name = \"demo\""));

    let remove_empty = OpKind::TomlRemove { toml_path: vec![] };
    let out2 = apply_op_to_content(&out, &remove_empty).expect("remove empty");
    assert_eq!(out, out2);
}

#[test]
fn apply_op_to_content_use_workspace_dependency_preserves_fields() {
    let contents = "[dependencies]\nserde = \"1.0\"\n";
    let kind = OpKind::TomlTransform {
        rule_id: "use_workspace_dependency".to_string(),
        args: Some(serde_json::json!({
            "toml_path": ["dependencies", "serde"],
            "preserved": {
                "package": "serde1",
                "optional": true,
                "default_features": false,
                "features": ["std", "derive"]
            }
        })),
    };

    let out = apply_op_to_content(contents, &kind).expect("apply");
    assert!(out.contains("workspace = true"));
    assert!(out.contains("package = \"serde1\""));
    assert!(out.contains("optional = true"));
    assert!(out.contains("default-features = false"));
    assert!(out.contains("features = [\"std\", \"derive\"]"));
}

#[test]
fn execute_plan_from_contents_applies_only_allowed_and_fills_params() {
    let mut plan = base_plan();
    plan.ops.push(make_op(
        "edition",
        "Cargo.toml",
        SafetyClass::Safe,
        false,
        OpKind::TomlTransform {
            rule_id: "set_package_edition".to_string(),
            args: None,
        },
        vec!["edition".to_string()],
    ));
    plan.ops.push(make_op(
        "path_dep",
        "Cargo.toml",
        SafetyClass::Safe,
        false,
        OpKind::TomlTransform {
            rule_id: "ensure_path_dep_has_version".to_string(),
            args: Some(serde_json::json!({
                "toml_path": ["dependencies", "dep"],
                "dep_path": "../dep"
            })),
        },
        vec!["version".to_string()],
    ));
    plan.ops.push(make_op(
        "blocked",
        "Cargo.toml",
        SafetyClass::Guarded,
        false,
        OpKind::TomlSet {
            toml_path: vec!["package".to_string(), "name".to_string()],
            value: serde_json::Value::String("blocked".to_string()),
        },
        vec![],
    ));
    plan.ops.push(make_op(
        "toml_set_with_params",
        "Cargo.toml",
        SafetyClass::Safe,
        false,
        OpKind::TomlSet {
            toml_path: vec![
                "package".to_string(),
                "metadata".to_string(),
                "flag".to_string(),
            ],
            value: serde_json::Value::Bool(true),
        },
        vec!["ignored".to_string()],
    ));

    let mut params = HashMap::new();
    params.insert("edition".to_string(), "2021".to_string());
    params.insert("version".to_string(), "1.2.3".to_string());
    params.insert("ignored".to_string(), "x".to_string());

    let opts = ApplyOptions {
        dry_run: true,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params,
    };

    let mut before = BTreeMap::new();
    before.insert(
        Utf8PathBuf::from("Cargo.toml"),
        "[package]\nname = \"demo\"\n\n[dependencies]\ndep = { path = \"../dep\" }\n".to_string(),
    );

    let changed =
        buildfix_edit::execute_plan_from_contents(&before, &plan, &opts).expect("execute");
    let out = changed.get(Utf8Path::new("Cargo.toml")).expect("changed");
    assert!(out.contains("edition = \"2021\""));
    assert!(out.contains("version = \"1.2.3\""));
    assert!(out.contains("flag = true"));
    assert!(!out.contains("name = \"blocked\""));
}

#[test]
fn apply_plan_handles_head_sha_mismatch() {
    let temp = TempDir::new().expect("temp dir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");

    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write");
    run_git(&root, &["init"]);
    run_git(&root, &["config", "user.email", "test@example.com"]);
    run_git(&root, &["config", "user.name", "Test User"]);
    run_git(&root, &["add", "."]);
    run_git(&root, &["commit", "-m", "init"]);

    let mut plan = base_plan();
    plan.preconditions.head_sha = Some("deadbeef".to_string());
    plan.ops.push(make_op(
        "op1",
        "Cargo.toml",
        SafetyClass::Safe,
        false,
        OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        vec![],
    ));

    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).expect("apply");
    assert!(!apply.preconditions.verified);
    assert!(
        apply
            .preconditions
            .mismatches
            .iter()
            .any(|m| m.path == "<git_head>")
    );
}

#[test]
fn apply_plan_allows_backup_enabled_without_dir() {
    let temp = TempDir::new().expect("temp dir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");

    fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write");
    let mut plan = base_plan();
    plan.ops.push(make_op(
        "op1",
        "Cargo.toml",
        SafetyClass::Safe,
        false,
        OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        vec![],
    ));

    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: true,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).expect("apply");
    let result = apply.results.iter().find(|r| r.op_id == "op1").unwrap();
    assert!(result.files.iter().all(|f| f.backup_path.is_none()));
}

#[test]
fn apply_plan_supports_absolute_paths() {
    let temp = TempDir::new().expect("temp dir");
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");

    let file_path = root.join("Cargo.toml");
    fs::write(&file_path, "[workspace]\n").expect("write");

    let mut plan = base_plan();
    plan.ops.push(make_op(
        "op1",
        file_path.as_str(),
        SafetyClass::Safe,
        false,
        OpKind::TomlTransform {
            rule_id: "ensure_workspace_resolver_v2".to_string(),
            args: None,
        },
        vec![],
    ));

    let opts = ApplyOptions {
        dry_run: false,
        allow_guarded: false,
        allow_unsafe: false,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: ".bak".to_string(),
        params: HashMap::new(),
    };

    let (_apply, _patch) = apply_plan(&root, &plan, tool_info(), &opts).expect("apply");
    let contents = fs::read_to_string(&file_path).expect("read");
    assert!(contents.contains("resolver = \"2\""));
}

#[test]
fn apply_op_to_content_path_dep_version_table_and_mismatch() {
    let inline = r#"
[dependencies]
dep = { path = "../dep" }
"#;
    let kind_mismatch = OpKind::TomlTransform {
        rule_id: "ensure_path_dep_has_version".to_string(),
        args: Some(serde_json::json!({
            "toml_path": ["dependencies", "dep"],
            "dep_path": "../other",
            "version": "1.2.3"
        })),
    };
    let out = apply_op_to_content(inline, &kind_mismatch).expect("apply");
    assert!(!out.contains("version"));

    let table = r#"
[dependencies.dep]
path = "../dep"
"#;
    let kind_table = OpKind::TomlTransform {
        rule_id: "ensure_path_dep_has_version".to_string(),
        args: Some(serde_json::json!({
            "toml_path": ["dependencies", "dep"],
            "dep_path": "../dep",
            "version": "1.2.3"
        })),
    };
    let out = apply_op_to_content(table, &kind_table).expect("apply");
    assert!(out.contains("version = \"1.2.3\""));
}

#[test]
fn apply_op_to_content_errors_for_short_toml_paths() {
    let kind_short = OpKind::TomlTransform {
        rule_id: "ensure_path_dep_has_version".to_string(),
        args: Some(serde_json::json!({
            "toml_path": ["dependencies"],
            "dep_path": "../dep",
            "version": "1.2.3"
        })),
    };
    let err = apply_op_to_content("", &kind_short).expect_err("short path");
    assert!(err.to_string().contains("dependency not found"));

    let kind_target_short = OpKind::TomlTransform {
        rule_id: "ensure_path_dep_has_version".to_string(),
        args: Some(serde_json::json!({
            "toml_path": ["target", "cfg(windows)", "dependencies"],
            "dep_path": "../dep",
            "version": "1.2.3"
        })),
    };
    let err = apply_op_to_content("", &kind_target_short).expect_err("short target path");
    assert!(err.to_string().contains("dependency not found"));
}
