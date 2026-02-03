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
}

fn repo_root(world: &BuildfixWorld) -> &Utf8PathBuf {
    world.repo_root.as_ref().expect("repo_root set")
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

#[then("the plan contains a resolver v2 fix")]
async fn assert_plan_contains_fix(world: &mut BuildfixWorld) {
    let root = repo_root(world).clone();
    let plan_path = root.join("artifacts").join("buildfix").join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).unwrap();
    let v: serde_json::Value = serde_json::from_str(&plan_str).unwrap();

    let fixes = v["fixes"].as_array().unwrap();
    assert!(
        fixes
            .iter()
            .any(|f| f["fix_id"] == "cargo.workspace_resolver_v2"),
        "expected a resolver v2 fix"
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

#[tokio::main]
async fn main() {
    let features_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("features");
    BuildfixWorld::cucumber()
        .run(features_path)
        .await;
}
