use anyhow::Context;
use assert_cmd::Command;
use camino::Utf8PathBuf;
use cucumber::{given, then, when, World};
use fs_err as fs;
use tempfile::TempDir;

#[derive(Debug, Default, World)]
pub struct BuildfixWorld {
    temp: Option<TempDir>,
    repo_root: Option<Utf8PathBuf>,
    explain_output: Option<String>,
}

fn repo_root(world: &BuildfixWorld) -> &Utf8PathBuf {
    world.repo_root.as_ref().expect("repo_root set")
}

fn plan_ops(plan: &serde_json::Value) -> &Vec<serde_json::Value> {
    plan["ops"].as_array().expect("ops array")
}

fn plan_has_rule(plan: &serde_json::Value, rule_id: &str) -> bool {
    plan_ops(plan).iter().any(|op| {
        op["kind"]["type"] == "toml_transform" && op["kind"]["rule_id"] == rule_id
    })
}

#[given("a repo missing workspace resolver v2")]
async fn repo_missing_resolver(world: &mut BuildfixWorld) {
    let td = tempfile::tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();

    // Minimal workspace with a member.
    fs::create_dir_all(root.join("crates").join("a")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/a"]
"#,
    )
    .unwrap();
    fs::write(
        root.join("crates").join("a").join("Cargo.toml"),
        r#"
[package]
name = "a"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    world.temp = Some(td);
    world.repo_root = Some(root);
}

#[given("a builddiag receipt for resolver v2")]
async fn builddiag_receipt(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let artifacts = root.join("artifacts").join("builddiag");
    fs::create_dir_all(&artifacts).unwrap();

    // Minimal receipt envelope.
    let receipt = serde_json::json!({
        "schema": "builddiag.report.v1",
        "tool": { "name": "builddiag", "version": "0.0.0" },
        "verdict": { "status": "fail", "counts": { "findings": 1, "errors": 1, "warnings": 0 } },
        "findings": [{
            "severity": "error",
            "check_id": "workspace.resolver_v2",
            "code": "not_v2",
            "message": "workspace resolver is not 2",
            "location": { "path": "Cargo.toml", "line": 1, "column": 1 }
        }]
    });

    fs::write(
        artifacts.join("report.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[when("I run buildfix plan")]
async fn run_plan(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str())
        .arg("plan")
        .assert()
        .success();
}

#[when("I run buildfix plan expecting policy block")]
async fn run_plan_expect_policy_block(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str())
        .arg("plan")
        .assert()
        .code(2);
}

#[then("the plan contains a resolver v2 fix")]
async fn assert_plan_contains_fix(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    assert!(
        plan_has_rule(&v, "ensure_workspace_resolver_v2"),
        "expected a resolver v2 op"
    );
}

#[when("I run buildfix apply with --apply")]
async fn run_apply(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str())
        .arg("apply")
        .arg("--apply")
        .assert()
        .success();
}

#[then(expr = "the root Cargo.toml sets workspace resolver to {string}")]
async fn assert_root_manifest_resolver(world: &mut BuildfixWorld, expected: String) {
    let root = repo_root(world).clone();
    let contents = fs::read_to_string(root.join("Cargo.toml"))
        .context("read Cargo.toml")
        .unwrap();
    assert!(
        contents.contains(&format!("resolver = \"{}\"", expected)),
        "expected resolver = \"{}\" in Cargo.toml, got:\n{}",
        expected,
        contents
    );
}

// ============================================================================
// Scenario: Adds version to path dependency
// ============================================================================

#[given("a repo with a path dependency missing version")]
async fn repo_with_path_dep_missing_version(world: &mut BuildfixWorld) {
    let td = tempfile::tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();

    // Create workspace with two crates
    fs::create_dir_all(root.join("crates").join("crate-a")).unwrap();
    fs::create_dir_all(root.join("crates").join("crate-b")).unwrap();

    // Root workspace manifest
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/crate-a", "crates/crate-b"]
resolver = "2"
"#,
    )
    .unwrap();

    // crate-b with a version
    fs::write(
        root.join("crates").join("crate-b").join("Cargo.toml"),
        r#"
[package]
name = "crate-b"
version = "0.2.0"
edition = "2021"
"#,
    )
    .unwrap();

    // crate-a depends on crate-b via path WITHOUT version
    fs::write(
        root.join("crates").join("crate-a").join("Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"

[dependencies]
crate-b = { path = "../crate-b" }
"#,
    )
    .unwrap();

    world.temp = Some(td);
    world.repo_root = Some(root);
}

#[given("a repo with a path dependency missing version and no target version")]
async fn repo_with_path_dep_missing_version_no_target_version(world: &mut BuildfixWorld) {
    let td = tempfile::tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();

    // Create workspace with two crates
    fs::create_dir_all(root.join("crates").join("crate-a")).unwrap();
    fs::create_dir_all(root.join("crates").join("crate-b")).unwrap();

    // Root workspace manifest (no workspace.package.version)
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/crate-a", "crates/crate-b"]
resolver = "2"
"#,
    )
    .unwrap();

    // crate-b without a version
    fs::write(
        root.join("crates").join("crate-b").join("Cargo.toml"),
        r#"
[package]
name = "crate-b"
edition = "2021"
"#,
    )
    .unwrap();

    // crate-a depends on crate-b via path WITHOUT version
    fs::write(
        root.join("crates").join("crate-a").join("Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"

[dependencies]
crate-b = { path = "../crate-b" }
"#,
    )
    .unwrap();

    world.temp = Some(td);
    world.repo_root = Some(root);
}

#[given("a depguard receipt for missing path dependency version")]
async fn depguard_receipt_path_dep(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let artifacts = root.join("artifacts").join("depguard");
    fs::create_dir_all(&artifacts).unwrap();

    let receipt = serde_json::json!({
        "schema": "depguard.report.v1",
        "tool": { "name": "depguard", "version": "0.0.0" },
        "verdict": { "status": "fail", "counts": { "findings": 1, "errors": 1, "warnings": 0 } },
        "findings": [{
            "severity": "error",
            "check_id": "deps.path_requires_version",
            "code": "missing_version",
            "message": "path dependency missing version",
            "location": { "path": "crates/crate-a/Cargo.toml", "line": 9, "column": 1 },
            "data": {
                "dep": "crate-b",
                "dep_path": "../crate-b",
                "toml_path": ["dependencies", "crate-b"]
            }
        }]
    });

    fs::write(
        artifacts.join("report.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[then("the plan contains a path dep version fix")]
async fn assert_plan_contains_path_dep_fix(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    assert!(
        plan_has_rule(&v, "ensure_path_dep_has_version"),
        "expected a path dep version op"
    );
}

#[then("the crate-a Cargo.toml has version for crate-b dependency")]
async fn assert_crate_a_has_version(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let contents = fs::read_to_string(root.join("crates").join("crate-a").join("Cargo.toml"))
        .context("read crate-a Cargo.toml")
        .unwrap();
    // Should have version = "0.2.0" for crate-b
    assert!(
        contents.contains("version = \"0.2.0\"") || contents.contains("version=\"0.2.0\""),
        "expected crate-b dependency to have version, got:\n{}",
        contents
    );
}

// ============================================================================
// Scenario: Converts to workspace dependency
// ============================================================================

#[given("a repo with a duplicate workspace dependency")]
async fn repo_with_duplicate_workspace_dep(world: &mut BuildfixWorld) {
    let td = tempfile::tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();

    fs::create_dir_all(root.join("crates").join("crate-a")).unwrap();

    // Root workspace with serde in workspace.dependencies
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/crate-a"]
resolver = "2"

[workspace.dependencies]
serde = "1.0"
"#,
    )
    .unwrap();

    // crate-a has its own serde = "1.0" instead of workspace = true
    fs::write(
        root.join("crates").join("crate-a").join("Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
"#,
    )
    .unwrap();

    world.temp = Some(td);
    world.repo_root = Some(root);
}

#[given("a depguard receipt for workspace inheritance")]
async fn depguard_receipt_workspace_inheritance(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let artifacts = root.join("artifacts").join("depguard");
    fs::create_dir_all(&artifacts).unwrap();

    let receipt = serde_json::json!({
        "schema": "depguard.report.v1",
        "tool": { "name": "depguard", "version": "0.0.0" },
        "verdict": { "status": "fail", "counts": { "findings": 1, "errors": 1, "warnings": 0 } },
        "findings": [{
            "severity": "error",
            "check_id": "deps.workspace_inheritance",
            "code": "should_use_workspace",
            "message": "dependency should use workspace inheritance",
            "location": { "path": "crates/crate-a/Cargo.toml", "line": 8, "column": 1 },
            "data": {
                "dep": "serde",
                "toml_path": ["dependencies", "serde"]
            }
        }]
    });

    fs::write(
        artifacts.join("report.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[then("the plan contains a workspace inheritance fix")]
async fn assert_plan_contains_workspace_inheritance_fix(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    assert!(
        plan_has_rule(&v, "use_workspace_dependency"),
        "expected a workspace inheritance op"
    );
}

#[then("the crate-a Cargo.toml uses workspace dependency for serde")]
async fn assert_crate_a_uses_workspace_serde(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let contents = fs::read_to_string(root.join("crates").join("crate-a").join("Cargo.toml"))
        .context("read crate-a Cargo.toml")
        .unwrap();
    // Should have workspace = true for serde
    assert!(
        contents.contains("workspace = true"),
        "expected serde to use workspace = true, got:\n{}",
        contents
    );
}

// ============================================================================
// Scenario: Normalizes MSRV to workspace value
// ============================================================================

#[given("a repo with inconsistent MSRV")]
async fn repo_with_inconsistent_msrv(world: &mut BuildfixWorld) {
    let td = tempfile::tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();

    fs::create_dir_all(root.join("crates").join("crate-a")).unwrap();

    // Root workspace with rust-version = "1.70"
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/crate-a"]
resolver = "2"

[workspace.package]
rust-version = "1.70"
"#,
    )
    .unwrap();

    // crate-a has older rust-version = "1.65"
    fs::write(
        root.join("crates").join("crate-a").join("Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"
rust-version = "1.65"
"#,
    )
    .unwrap();

    world.temp = Some(td);
    world.repo_root = Some(root);
}

#[given("a builddiag receipt for MSRV inconsistency")]
async fn builddiag_receipt_msrv(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let artifacts = root.join("artifacts").join("builddiag");
    fs::create_dir_all(&artifacts).unwrap();

    let receipt = serde_json::json!({
        "schema": "builddiag.report.v1",
        "tool": { "name": "builddiag", "version": "0.0.0" },
        "verdict": { "status": "fail", "counts": { "findings": 1, "errors": 1, "warnings": 0 } },
        "findings": [{
            "severity": "error",
            "check_id": "rust.msrv_consistent",
            "code": "msrv_mismatch",
            "message": "crate MSRV does not match workspace",
            "location": { "path": "crates/crate-a/Cargo.toml", "line": 6, "column": 1 },
            "data": {
                "crate_msrv": "1.65",
                "workspace_msrv": "1.70"
            }
        }]
    });

    fs::write(
        artifacts.join("report.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[then("the plan contains an MSRV normalization fix")]
async fn assert_plan_contains_msrv_fix(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    assert!(
        plan_has_rule(&v, "set_package_rust_version"),
        "expected an MSRV normalization op"
    );
}

#[when("I run buildfix apply with --apply --allow-guarded")]
async fn run_apply_allow_guarded(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str())
        .arg("apply")
        .arg("--apply")
        .arg("--allow-guarded")
        .assert()
        .success();
}

#[then(expr = "the crate-a Cargo.toml has rust-version {string}")]
async fn assert_crate_a_has_msrv(world: &mut BuildfixWorld, expected: String) {
    let root = repo_root(world).clone();
    let contents = fs::read_to_string(root.join("crates").join("crate-a").join("Cargo.toml"))
        .context("read crate-a Cargo.toml")
        .unwrap();
    let expected_line = format!("rust-version = \"{}\"", expected);
    assert!(
        contents.contains(&expected_line),
        "expected rust-version = \"{}\", got:\n{}",
        expected,
        contents
    );
}

// ============================================================================
// Scenario: Guarded fix skipped without --allow-guarded flag
// ============================================================================

#[then(expr = "the crate-a Cargo.toml still has rust-version {string}")]
async fn assert_crate_a_still_has_msrv(world: &mut BuildfixWorld, expected: String) {
    let root = repo_root(world).clone();
    let contents = fs::read_to_string(root.join("crates").join("crate-a").join("Cargo.toml"))
        .context("read crate-a Cargo.toml")
        .unwrap();
    let expected_line = format!("rust-version = \"{}\"", expected);
    assert!(
        contents.contains(&expected_line),
        "expected rust-version still \"{}\" (not changed), got:\n{}",
        expected,
        contents
    );
}

// ============================================================================
// Scenario: Dry-run apply does not modify files
// ============================================================================

#[when("I run buildfix apply without --apply")]
async fn run_apply_dry_run(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str())
        .arg("apply")
        .assert()
        .success();
}

#[then("the root Cargo.toml does not have workspace resolver")]
async fn assert_root_manifest_no_resolver(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let contents = fs::read_to_string(root.join("Cargo.toml"))
        .context("read Cargo.toml")
        .unwrap();
    assert!(
        !contents.contains("resolver ="),
        "expected no resolver in Cargo.toml (dry-run should not modify), got:\n{}",
        contents
    );
}

// ============================================================================
// Scenario: Empty plan when no matching receipts
// ============================================================================

#[given("an empty artifacts directory")]
async fn empty_artifacts_directory(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let artifacts = root.join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    // No sensor directories or receipts
}

#[then("the plan contains no fixes")]
async fn assert_plan_empty(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    let ops = plan_ops(&v);
    assert!(ops.is_empty(), "expected empty plan, got {} ops", ops.len());
}

// ============================================================================
// Scenario: Plan fails when max_ops cap exceeded
// ============================================================================

#[when("I run buildfix plan with --max-ops 0")]
async fn run_plan_with_max_ops_zero(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str())
        .arg("plan")
        .arg("--max-ops")
        .arg("0")
        .assert()
        .failure();
}

#[when(expr = "I run buildfix plan with allowlist {string}")]
async fn run_plan_with_allowlist(world: &mut BuildfixWorld, pattern: String) {
    let root = repo_root(world).clone();
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str())
        .arg("plan")
        .arg("--allow")
        .arg(pattern)
        .assert()
        .code(2);
}

#[then("the resolver v2 op is blocked by allowlist")]
async fn assert_resolver_op_blocked_by_allowlist(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    let op = plan_ops(&v)
        .iter()
        .find(|op| op["kind"]["type"] == "toml_transform" && op["kind"]["rule_id"] == "ensure_workspace_resolver_v2")
        .expect("resolver v2 op");

    assert_eq!(op["blocked"].as_bool(), Some(true));
    let reason = op["blocked_reason"].as_str().unwrap_or("");
    assert!(
        reason.contains("allowlist"),
        "expected allowlist block reason, got: {}",
        reason
    );
}

#[then("the path dependency version op is blocked for missing params")]
async fn assert_path_dep_blocked_missing_params(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    let op = plan_ops(&v)
        .iter()
        .find(|op| op["kind"]["type"] == "toml_transform" && op["kind"]["rule_id"] == "ensure_path_dep_has_version")
        .expect("path dep op");

    assert_eq!(op["blocked"].as_bool(), Some(true));
    assert_eq!(op["safety"].as_str(), Some("unsafe"));
    let reason = op["blocked_reason"].as_str().unwrap_or("");
    assert!(
        reason.contains("missing params"),
        "expected missing params reason, got: {}",
        reason
    );
    let params = op["params_required"].as_array().cloned().unwrap_or_default();
    assert!(
        params.iter().any(|v| v.as_str() == Some("version")),
        "expected params_required to contain version"
    );
}

#[when("I modify the root Cargo.toml after planning")]
async fn modify_root_manifest_after_plan(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let path = root.join("Cargo.toml");
    let mut contents = fs::read_to_string(&path).unwrap_or_default();
    contents.push_str("\n# modified after plan\n");
    fs::write(&path, contents).unwrap();
}

#[when("I run buildfix apply with --apply expecting policy block")]
async fn run_apply_expect_policy_block(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str())
        .arg("apply")
        .arg("--apply")
        .assert()
        .code(2);
}

#[then("the apply preconditions are not verified")]
async fn assert_apply_preconditions_not_verified(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let apply_path = root.join("artifacts").join("buildfix").join("apply.json");
    let apply_str = fs::read_to_string(&apply_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&apply_str).unwrap();

    assert_eq!(v["preconditions"]["verified"].as_bool(), Some(false));
    let mismatches = v["preconditions"]["mismatches"].as_array().cloned().unwrap_or_default();
    assert!(
        !mismatches.is_empty(),
        "expected at least one precondition mismatch"
    );
}

#[then("the plan command fails")]
async fn assert_plan_fails(_world: &mut BuildfixWorld) {
    // The failure is asserted in the when step
}

// ============================================================================
// Scenario: Multiple fixes on same manifest produce stable output
// ============================================================================

#[given("a repo with multiple issues")]
async fn repo_with_multiple_issues(world: &mut BuildfixWorld) {
    let td = tempfile::tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();

    fs::create_dir_all(root.join("crates").join("crate-a")).unwrap();
    fs::create_dir_all(root.join("crates").join("crate-b")).unwrap();

    // Root workspace: missing resolver, has workspace.dependencies
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/crate-a", "crates/crate-b"]

[workspace.dependencies]
serde = "1.0"
"#,
    )
    .unwrap();

    // crate-b with a version
    fs::write(
        root.join("crates").join("crate-b").join("Cargo.toml"),
        r#"
[package]
name = "crate-b"
version = "0.2.0"
edition = "2021"
"#,
    )
    .unwrap();

    // crate-a: path dep without version + duplicate workspace dep
    fs::write(
        root.join("crates").join("crate-a").join("Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"

[dependencies]
crate-b = { path = "../crate-b" }
serde = "1.0"
"#,
    )
    .unwrap();

    world.temp = Some(td);
    world.repo_root = Some(root);
}

#[given("receipts for multiple issues")]
async fn receipts_for_multiple_issues(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();

    // builddiag receipt for resolver v2
    let builddiag = root.join("artifacts").join("builddiag");
    fs::create_dir_all(&builddiag).unwrap();
    let receipt1 = serde_json::json!({
        "schema": "builddiag.report.v1",
        "tool": { "name": "builddiag", "version": "0.0.0" },
        "verdict": { "status": "fail", "counts": { "findings": 1, "errors": 1, "warnings": 0 } },
        "findings": [{
            "severity": "error",
            "check_id": "workspace.resolver_v2",
            "code": "not_v2",
            "message": "workspace resolver is not 2",
            "location": { "path": "Cargo.toml", "line": 1, "column": 1 }
        }]
    });
    fs::write(
        builddiag.join("report.json"),
        serde_json::to_string_pretty(&receipt1).unwrap(),
    )
    .unwrap();

    // depguard receipt for path dep version and workspace inheritance
    let depguard = root.join("artifacts").join("depguard");
    fs::create_dir_all(&depguard).unwrap();
    let receipt2 = serde_json::json!({
        "schema": "depguard.report.v1",
        "tool": { "name": "depguard", "version": "0.0.0" },
        "verdict": { "status": "fail", "counts": { "findings": 2, "errors": 2, "warnings": 0 } },
        "findings": [
            {
                "severity": "error",
                "check_id": "deps.path_requires_version",
                "code": "missing_version",
                "message": "path dependency missing version",
                "location": { "path": "crates/crate-a/Cargo.toml", "line": 9, "column": 1 },
                "data": {
                    "dep": "crate-b",
                    "dep_path": "../crate-b",
                    "toml_path": ["dependencies", "crate-b"]
                }
            },
            {
                "severity": "error",
                "check_id": "deps.workspace_inheritance",
                "code": "should_use_workspace",
                "message": "dependency should use workspace inheritance",
                "location": { "path": "crates/crate-a/Cargo.toml", "line": 10, "column": 1 },
                "data": {
                    "dep": "serde",
                    "toml_path": ["dependencies", "serde"]
                }
            }
        ]
    });
    fs::write(
        depguard.join("report.json"),
        serde_json::to_string_pretty(&receipt2).unwrap(),
    )
    .unwrap();
}

#[then("the plan contains multiple fixes")]
async fn assert_plan_contains_multiple_fixes(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    let ops = plan_ops(&v);
    assert!(ops.len() >= 2, "expected multiple ops, got {}", ops.len());
}

#[then("the fixes are sorted deterministically")]
async fn assert_fixes_sorted_deterministically(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");

    // Run plan twice and compare
    let plan_str1 = fs::read_to_string(&plan_path).unwrap();

    // Run plan again
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str())
        .arg("plan")
        .assert()
        .success();

    let plan_str2 = fs::read_to_string(&plan_path).unwrap();

    // Parse and compare fix order
    let v1: serde_json::Value = serde_json::from_str(&plan_str1).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&plan_str2).unwrap();

    let fixes1: Vec<&str> = v1["ops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap())
        .collect();
    let fixes2: Vec<&str> = v2["ops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap())
        .collect();

    assert_eq!(
        fixes1, fixes2,
        "fix order should be deterministic across runs"
    );
}

// ============================================================================
// Scenario: Workspace inheritance preserves dependency features
// ============================================================================

#[given("a repo with workspace dep that has features")]
async fn repo_with_workspace_dep_features(world: &mut BuildfixWorld) {
    let td = tempfile::tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();

    fs::create_dir_all(root.join("crates").join("crate-a")).unwrap();

    // Root workspace with serde in workspace.dependencies
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/crate-a"]
resolver = "2"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
"#,
    )
    .unwrap();

    // crate-a has serde = { version = "1.0", features = ["derive"] }
    fs::write(
        root.join("crates").join("crate-a").join("Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
"#,
    )
    .unwrap();

    world.temp = Some(td);
    world.repo_root = Some(root);
}

#[given("a depguard receipt for workspace inheritance with features")]
async fn depguard_receipt_workspace_inheritance_features(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let artifacts = root.join("artifacts").join("depguard");
    fs::create_dir_all(&artifacts).unwrap();

    let receipt = serde_json::json!({
        "schema": "depguard.report.v1",
        "tool": { "name": "depguard", "version": "0.0.0" },
        "verdict": { "status": "fail", "counts": { "findings": 1, "errors": 1, "warnings": 0 } },
        "findings": [{
            "severity": "error",
            "check_id": "deps.workspace_inheritance",
            "code": "should_use_workspace",
            "message": "dependency should use workspace inheritance",
            "location": { "path": "crates/crate-a/Cargo.toml", "line": 8, "column": 1 },
            "data": {
                "dep": "serde",
                "toml_path": ["dependencies", "serde"]
            }
        }]
    });

    fs::write(
        artifacts.join("report.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[then("the crate-a Cargo.toml has workspace serde with features preserved")]
async fn assert_crate_a_workspace_serde_features(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let contents = fs::read_to_string(root.join("crates").join("crate-a").join("Cargo.toml"))
        .context("read crate-a Cargo.toml")
        .unwrap();
    // Should have workspace = true AND features = ["derive"]
    assert!(
        contents.contains("workspace = true"),
        "expected workspace = true, got:\n{}",
        contents
    );
    assert!(
        contents.contains("features = [\"derive\"]")
            || contents.contains("features = [ \"derive\" ]"),
        "expected features preserved, got:\n{}",
        contents
    );
}

// ============================================================================
// Scenario: Explain command describes a fix
// ============================================================================

#[when(expr = "I run buildfix explain {word}")]
async fn run_explain(world: &mut BuildfixWorld, fix_key: String) {
    // Explain command doesn't need a repo, so create minimal temp dir if none exists
    if world.temp.is_none() {
        let td = tempfile::tempdir().expect("tempdir");
        let root = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();
        world.temp = Some(td);
        world.repo_root = Some(root);
    }

    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    let output = cmd
        .arg("explain")
        .arg(&fix_key)
        .output()
        .expect("run explain");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // Store in world for later assertions
    world.explain_output = Some(stdout);
}

#[then("the output contains the fix description")]
async fn assert_explain_output(world: &mut BuildfixWorld) {
    let output = world.explain_output.as_ref().expect("explain output");
    assert!(
        output.contains("FIX:") && output.contains("DESCRIPTION"),
        "expected explain output to contain FIX: and DESCRIPTION, got:\n{}",
        output
    );
}

#[when("I run buildfix list-fixes")]
async fn run_list_fixes(world: &mut BuildfixWorld) {
    let output = Command::cargo_bin("buildfix")
        .unwrap()
        .arg("list-fixes")
        .output()
        .unwrap();
    world.explain_output = Some(String::from_utf8_lossy(&output.stdout).to_string());
}

#[when("I run buildfix list-fixes --format json")]
async fn run_list_fixes_json(world: &mut BuildfixWorld) {
    let output = Command::cargo_bin("buildfix")
        .unwrap()
        .arg("list-fixes")
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    world.explain_output = Some(String::from_utf8_lossy(&output.stdout).to_string());
}

#[then(expr = "the output contains {string}")]
async fn assert_output_contains(world: &mut BuildfixWorld, expected: String) {
    let output = world.explain_output.as_ref().expect("output");
    assert!(
        output.contains(&expected),
        "expected output to contain '{}', got:\n{}",
        expected,
        output
    );
}

#[then("the output is valid JSON")]
async fn assert_output_is_json(world: &mut BuildfixWorld) {
    let output = world.explain_output.as_ref().expect("output");
    let _: serde_json::Value = serde_json::from_str(output).expect("output should be valid JSON");
}

#[then(expr = "the JSON output contains fix with key {string}")]
async fn assert_json_contains_fix(world: &mut BuildfixWorld, key: String) {
    let output = world.explain_output.as_ref().expect("output");
    let json: serde_json::Value =
        serde_json::from_str(output).expect("output should be valid JSON");
    let fixes = json.as_array().expect("JSON should be an array");
    assert!(
        fixes.iter().any(|f| f["key"].as_str() == Some(&key)),
        "expected JSON to contain fix with key '{}', got:\n{}",
        key,
        output
    );
}

// ============================================================================
// Scenario: Re-running apply on fixed repo produces no changes
// ============================================================================

#[when("I regenerate receipts for the fixed repo")]
async fn regenerate_receipts_for_fixed_repo(world: &mut BuildfixWorld) {
    // After resolver v2 is applied, generate a receipt that shows no issues
    let root = repo_root(world).clone();
    let artifacts = root.join("artifacts").join("builddiag");
    fs::create_dir_all(&artifacts).unwrap();

    // Passing receipt - no findings
    let receipt = serde_json::json!({
        "schema": "builddiag.report.v1",
        "tool": { "name": "builddiag", "version": "0.0.0" },
        "verdict": { "status": "pass", "counts": { "findings": 0, "errors": 0, "warnings": 0 } },
        "findings": []
    });

    fs::write(
        artifacts.join("report.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

// ============================================================================
// Scenario: Converts dev-dependency to workspace inheritance
// ============================================================================

#[given("a repo with duplicate dev-dependency")]
async fn repo_with_duplicate_dev_dependency(world: &mut BuildfixWorld) {
    let td = tempfile::tempdir().expect("tempdir");
    let root = Utf8PathBuf::from_path_buf(td.path().to_path_buf()).unwrap();

    fs::create_dir_all(root.join("crates").join("crate-a")).unwrap();

    // Root workspace with tokio in workspace.dependencies
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/crate-a"]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1.0", features = ["rt"] }
"#,
    )
    .unwrap();

    // crate-a has tokio as dev-dependency (not using workspace)
    fs::write(
        root.join("crates").join("crate-a").join("Cargo.toml"),
        r#"
[package]
name = "crate-a"
version = "0.1.0"
edition = "2021"

[dev-dependencies]
tokio = { version = "1.0", features = ["rt"] }
"#,
    )
    .unwrap();

    world.temp = Some(td);
    world.repo_root = Some(root);
}

#[given("a depguard receipt for dev-dependency inheritance")]
async fn depguard_receipt_dev_dependency(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let artifacts = root.join("artifacts").join("depguard");
    fs::create_dir_all(&artifacts).unwrap();

    let receipt = serde_json::json!({
        "schema": "depguard.report.v1",
        "tool": { "name": "depguard", "version": "0.0.0" },
        "verdict": { "status": "fail", "counts": { "findings": 1, "errors": 1, "warnings": 0 } },
        "findings": [{
            "severity": "error",
            "check_id": "deps.workspace_inheritance",
            "code": "should_use_workspace",
            "message": "dev-dependency should use workspace inheritance",
            "location": { "path": "crates/crate-a/Cargo.toml", "line": 9, "column": 1 },
            "data": {
                "dep": "tokio",
                "toml_path": ["dev-dependencies", "tokio"]
            }
        }]
    });

    fs::write(
        artifacts.join("report.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[then("the crate-a Cargo.toml uses workspace dev-dependency for tokio")]
async fn assert_crate_a_workspace_dev_dep(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let contents = fs::read_to_string(root.join("crates").join("crate-a").join("Cargo.toml"))
        .context("read crate-a Cargo.toml")
        .unwrap();
    // Should have workspace = true for tokio in dev-dependencies
    assert!(
        contents.contains("[dev-dependencies]"),
        "expected [dev-dependencies] section, got:\n{}",
        contents
    );
    assert!(
        contents.contains("workspace = true"),
        "expected tokio to use workspace = true, got:\n{}",
        contents
    );
}

// ============================================================================
// Scenario: Plan produces valid JSON artifacts
// ============================================================================

#[then(expr = "the artifacts directory contains {word}")]
async fn assert_artifacts_contains_file(world: &mut BuildfixWorld, filename: String) {
    let root = repo_root(world).clone();
    let file_path = root.join("artifacts").join("buildfix").join(&filename);
    assert!(
        file_path.exists(),
        "expected {} to exist at {}",
        filename,
        file_path
    );
}

#[then("the plan.json has valid schema version")]
async fn assert_plan_json_schema(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    let schema = v["schema"].as_str().unwrap();
    assert_eq!(
        schema, "buildfix.plan.v1",
        "expected buildfix.plan.v1 schema, got: {}",
        schema
    );
}

// ============================================================================
// Scenario: Apply produces valid JSON artifacts
// ============================================================================

#[then("the apply.json has valid schema version")]
async fn assert_apply_json_schema(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let apply_path = root.join("artifacts").join("buildfix").join("apply.json");
    let apply_str = fs::read_to_string(&apply_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&apply_str).unwrap();

    let schema = v["schema"].as_str().unwrap();
    assert_eq!(
        schema, "buildfix.apply.v1",
        "expected buildfix.apply.v1 schema, got: {}",
        schema
    );
}

#[then("buildfix validate succeeds")]
async fn validate_succeeds(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let mut cmd = Command::cargo_bin("buildfix").expect("buildfix binary");
    cmd.current_dir(root.as_str()).arg("validate").assert().success();
}

#[tokio::main]
async fn main() {
    let features_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("features");
    BuildfixWorld::cucumber().run(features_path).await;
}
