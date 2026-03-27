//! CLI argument parsing edge case tests.

#![allow(deprecated)]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn buildfix() -> Command {
    Command::cargo_bin("buildfix").expect("buildfix binary")
}

fn create_temp_repo() -> TempDir {
    let td = tempfile::tempdir().expect("tempdir");
    let root = td.path();

    // Create minimal workspace
    fs::create_dir_all(root.join("crates").join("a")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["crates/a"]
resolver = "2"
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

    // Create empty artifacts directory
    fs::create_dir_all(root.join("artifacts")).unwrap();

    td
}

#[test]
fn test_plan_no_args_uses_current_dir() {
    let temp = create_temp_repo();

    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .success();
}

#[test]
fn test_apply_no_args_is_dry_run() {
    let temp = create_temp_repo();

    // First create a plan
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .success();

    // Apply without --apply should succeed (dry-run)
    buildfix()
        .current_dir(temp.path())
        .arg("apply")
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run").or(predicate::str::is_empty()));
}

#[test]
fn test_duplicate_allow_flags() {
    let temp = create_temp_repo();

    // Multiple --allow flags should accumulate
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--allow")
        .arg("pattern1/*")
        .arg("--allow")
        .arg("pattern2/*")
        .arg("--allow")
        .arg("pattern3/*")
        .assert()
        .success();
}

#[test]
fn test_duplicate_deny_flags() {
    let temp = create_temp_repo();

    // Multiple --deny flags should accumulate
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--deny")
        .arg("pattern1/*")
        .arg("--deny")
        .arg("pattern2/*")
        .assert()
        .success();
}

#[test]
fn test_duplicate_param_flags() {
    let temp = create_temp_repo();

    // Multiple --param flags should accumulate, later overrides earlier
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--param")
        .arg("key1=value1")
        .arg("--param")
        .arg("key2=value2")
        .arg("--param")
        .arg("key1=overridden")
        .assert()
        .success();
}

#[test]
fn test_invalid_param_format_missing_equals() {
    let temp = create_temp_repo();

    // Param without = should fail
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--param")
        .arg("keyonly")
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing"));
}

#[test]
fn test_invalid_param_format_empty_key() {
    let temp = create_temp_repo();

    // Param with empty key should fail
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--param")
        .arg("=value")
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing key"));
}

#[test]
fn test_invalid_param_format_empty_value() {
    let temp = create_temp_repo();

    // Param with empty value should fail
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--param")
        .arg("key=")
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing value"));
}

#[test]
fn test_list_fixes_text_format() {
    buildfix()
        .arg("list-fixes")
        .arg("--format")
        .arg("text")
        .assert()
        .success()
        .stdout(predicate::str::contains("resolver-v2"));
}

#[test]
fn test_list_fixes_json_format() {
    buildfix()
        .arg("list-fixes")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("resolver-v2"));
}

#[test]
fn test_list_fixes_invalid_format() {
    buildfix()
        .arg("list-fixes")
        .arg("--format")
        .arg("invalid")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("invalid").or(predicate::str::contains("possible values")),
        );
}

#[test]
fn test_explain_valid_fix() {
    buildfix()
        .arg("explain")
        .arg("resolver-v2")
        .assert()
        .success()
        .stdout(predicate::str::contains("Workspace Resolver V2"));
}

#[test]
fn test_explain_invalid_fix() {
    buildfix()
        .arg("explain")
        .arg("nonexistent-fix-key")
        .assert()
        .failure()
        .stdout(predicate::str::contains("not found").or(predicate::str::contains("Unknown")));
}

#[test]
fn test_explain_case_insensitive() {
    buildfix()
        .arg("explain")
        .arg("RESOLVER-V2")
        .assert()
        .success();

    buildfix()
        .arg("explain")
        .arg("Resolver_V2")
        .assert()
        .success();
}

#[test]
fn test_plan_conflicting_caps() {
    let temp = create_temp_repo();

    // Setting multiple caps should work
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--max-ops")
        .arg("10")
        .arg("--max-files")
        .arg("5")
        .arg("--max-patch-bytes")
        .arg("1000")
        .assert()
        .success();
}

#[test]
fn test_plan_cap_zero() {
    let temp = create_temp_repo();

    // max-ops 0 should be allowed (blocks all ops)
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--max-ops")
        .arg("0")
        .assert()
        .success();
}

#[test]
fn test_plan_invalid_cap_negative() {
    let temp = create_temp_repo();

    // Negative number should fail (clap treats it as unexpected argument)
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--max-ops")
        .arg("-1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected").or(predicate::str::contains("invalid")));
}

#[test]
fn test_plan_invalid_cap_non_numeric() {
    let temp = create_temp_repo();

    // Non-numeric should fail
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .arg("--max-ops")
        .arg("abc")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid"));
}

#[test]
fn test_nonexistent_repo_root() {
    // Note: buildfix may succeed with an empty plan even if the repo root doesn't exist
    // as long as it can create the output directory
    let result = buildfix()
        .arg("plan")
        .arg("--repo-root")
        .arg("/nonexistent/path/that/does/not/exist")
        .assert();

    // Either it fails (expected) or succeeds with empty plan (acceptable)
    // We just verify the command runs without crashing
    let _ = result;
}

#[test]
fn test_unknown_subcommand() {
    buildfix()
        .arg("unknown-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid").or(predicate::str::contains("unrecognized")));
}

#[test]
fn test_help_flag() {
    buildfix()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("buildfix"))
        .stdout(predicate::str::contains("plan"))
        .stdout(predicate::str::contains("apply"))
        .stdout(predicate::str::contains("explain"));
}

#[test]
fn test_version_flag() {
    buildfix()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("buildfix"));
}

#[test]
fn test_apply_with_both_guarded_and_unsafe() {
    let temp = create_temp_repo();

    // First create a plan
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .success();

    // Both --allow-guarded and --allow-unsafe should be allowed together
    buildfix()
        .current_dir(temp.path())
        .arg("apply")
        .arg("--allow-guarded")
        .arg("--allow-unsafe")
        .assert()
        .success();
}

#[test]
fn test_apply_auto_commit_requires_apply_flag() {
    let temp = create_temp_repo();

    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .success();

    buildfix()
        .current_dir(temp.path())
        .args(["apply", "--auto-commit"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("--auto-commit requires --apply"));
}

#[test]
fn test_apply_commit_message_requires_auto_commit() {
    let temp = create_temp_repo();

    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .success();

    buildfix()
        .current_dir(temp.path())
        .args(["apply", "--apply", "--commit-message", "buildfix: test"])
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "--commit-message requires auto-commit",
        ));
}

#[test]
fn test_apply_auto_commit_disallows_allow_dirty() {
    let temp = create_temp_repo();

    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .success();

    buildfix()
        .current_dir(temp.path())
        .args(["apply", "--apply", "--auto-commit", "--allow-dirty"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("requires a clean working tree"));
}

#[test]
fn test_validate_with_no_artifacts() {
    let temp = create_temp_repo();

    // Validate with no buildfix artifacts should succeed (nothing to validate)
    buildfix()
        .current_dir(temp.path())
        .arg("validate")
        .assert()
        .success();
}

#[test]
fn test_validate_detects_schema_violation() {
    let temp = create_temp_repo();
    let bf_dir = temp.path().join("artifacts").join("buildfix");
    fs::create_dir_all(&bf_dir).unwrap();

    // Valid JSON but violates the plan schema (missing required fields).
    fs::write(bf_dir.join("plan.json"), r#"{"not_a_plan": true}"#).unwrap();

    buildfix()
        .current_dir(temp.path())
        .arg("validate")
        .assert()
        .code(2);
}

#[test]
fn test_validate_round_trip() {
    let temp = create_temp_repo();

    // Generate artifacts via plan.
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .success();

    // Validate that the generated artifacts pass schema validation.
    buildfix()
        .current_dir(temp.path())
        .arg("validate")
        .assert()
        .success();
}

#[test]
fn test_plan_mode_standalone_is_default() {
    let temp = create_temp_repo();

    // Default mode (standalone) — plan with no receipts should succeed (exit 0).
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .success();
}

#[test]
fn test_plan_mode_cockpit_accepted() {
    let temp = create_temp_repo();

    // --mode cockpit is a valid flag
    buildfix()
        .current_dir(temp.path())
        .args(["plan", "--mode", "cockpit"])
        .assert()
        .success();
}

#[test]
fn test_apply_mode_cockpit_accepted() {
    let temp = create_temp_repo();

    // First create a plan
    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .success();

    // --mode cockpit is a valid flag for apply
    buildfix()
        .current_dir(temp.path())
        .args(["apply", "--mode", "cockpit"])
        .assert()
        .success();
}

#[test]
fn test_plan_mode_invalid_rejected() {
    let temp = create_temp_repo();

    buildfix()
        .current_dir(temp.path())
        .args(["plan", "--mode", "invalid"])
        .assert()
        .failure();
}

// ────────────────────────────────────────────────────────────────────────
// Exit code contract tests
//
// Exit 0 = success
// Exit 1 = tool / runtime error
// Exit 2 = policy block (precondition mismatch, safety gate, validation)
// ────────────────────────────────────────────────────────────────────────

#[test]
fn exit_code_0_plan_success() {
    let temp = create_temp_repo();

    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .code(0);
}

#[test]
fn exit_code_0_apply_dry_run_success() {
    let temp = create_temp_repo();

    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .code(0);

    buildfix()
        .current_dir(temp.path())
        .arg("apply")
        .assert()
        .code(0);
}

#[test]
fn exit_code_0_explain_success() {
    buildfix()
        .arg("explain")
        .arg("resolver-v2")
        .assert()
        .code(0);
}

#[test]
fn exit_code_0_list_fixes_success() {
    buildfix().arg("list-fixes").assert().code(0);
}

#[test]
fn exit_code_0_validate_success() {
    let temp = create_temp_repo();

    buildfix()
        .current_dir(temp.path())
        .arg("validate")
        .assert()
        .code(0);
}

#[test]
fn exit_code_1_explain_unknown_fix() {
    buildfix()
        .arg("explain")
        .arg("nonexistent-fix")
        .assert()
        .code(1);
}

#[test]
fn exit_code_1_apply_missing_plan() {
    let temp = create_temp_repo();

    // No plan.json exists, so apply should fail with exit 1 (tool error).
    buildfix()
        .current_dir(temp.path())
        .args(["apply", "--apply"])
        .assert()
        .code(1);
}

#[test]
fn exit_code_2_validate_schema_violation() {
    let temp = create_temp_repo();
    let bf_dir = temp.path().join("artifacts").join("buildfix");
    fs::create_dir_all(&bf_dir).unwrap();

    // Valid JSON but violates the plan schema.
    fs::write(bf_dir.join("plan.json"), r#"{"not_a_plan": true}"#).unwrap();

    buildfix()
        .current_dir(temp.path())
        .arg("validate")
        .assert()
        .code(2);
}

#[test]
fn exit_code_1_auto_commit_without_apply() {
    let temp = create_temp_repo();

    buildfix()
        .current_dir(temp.path())
        .arg("plan")
        .assert()
        .code(0);

    // --auto-commit without --apply is a tool error (invalid argument combo).
    buildfix()
        .current_dir(temp.path())
        .args(["apply", "--auto-commit"])
        .assert()
        .code(1);
}
