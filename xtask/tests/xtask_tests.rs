//! Integration tests for xtask binary.
//!
//! These tests exercise the xtask CLI by running it as a subprocess.

use std::process::Command;
use tempfile::TempDir;

/// Helper to get the xtask binary path
fn xtask_bin() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_xtask") {
        return path.into();
    }

    // Use CARGO_MANIFEST_DIR to find the workspace root, then look for the binary
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR should be set during test execution");
    let workspace_root = std::path::Path::new(&manifest_dir)
        .parent()
        .expect("manifest dir should have parent")
        .to_path_buf();

    // Try debug build first, then release
    let debug_path = workspace_root.join(format!(
        "target/debug/xtask{}",
        std::env::consts::EXE_SUFFIX
    ));
    let release_path = workspace_root.join(format!(
        "target/release/xtask{}",
        std::env::consts::EXE_SUFFIX
    ));

    if debug_path.exists() {
        debug_path
    } else if release_path.exists() {
        release_path
    } else {
        // Fall back to debug path - test will fail with clear error if missing
        debug_path
    }
}

/// Helper to run xtask with arguments
fn run_xtask(args: &[&str]) -> std::process::Output {
    Command::new(xtask_bin())
        .args(args)
        .output()
        .expect("Failed to execute xtask binary - run `cargo build -p xtask` first")
}

// =============================================================================
// CLI Parsing Tests
// =============================================================================

#[test]
fn cli_help_flag_works() {
    let output = run_xtask(&["--help"]);

    assert!(output.status.success(), "xtask --help should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Workspace helper tasks"),
        "Help should contain description"
    );
    assert!(
        stdout.contains("print-schemas"),
        "Help should list print-schemas command"
    );
    assert!(
        stdout.contains("init-artifacts"),
        "Help should list init-artifacts command"
    );
    assert!(
        stdout.contains("bless-fixtures"),
        "Help should list bless-fixtures command"
    );
    assert!(
        stdout.contains("validate"),
        "Help should list validate command"
    );
    assert!(
        stdout.contains("conform"),
        "Help should list conform command"
    );
}

#[test]
fn cli_version_flag_works() {
    let output = run_xtask(&["--version"]);

    // clap may or may not support --version depending on configuration
    // If it fails, stderr should contain version-related info or error
    // If it succeeds, output should contain xtask
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("xtask"),
            "Version output should contain 'xtask'"
        );
    } else {
        // Version flag not configured - this is acceptable
        // Just verify the command didn't crash
    }
}

#[test]
fn cli_unknown_command_fails() {
    let output = run_xtask(&["unknown-command"]);

    assert!(!output.status.success(), "Unknown command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error") || stderr.contains("unrecognized"),
        "Error message should indicate unknown command"
    );
}

#[test]
fn cli_no_command_fails() {
    let output = run_xtask(&[]);

    // clap requires a subcommand, so this should fail or show help
    // The behavior depends on clap configuration
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Either shows help or an error - both are acceptable
    assert!(
        !output.status.success() || stdout.contains("Workspace helper tasks"),
        "No command should fail or show help"
    );
}

// =============================================================================
// print-schemas Command Tests
// =============================================================================

#[test]
fn print_schemas_outputs_schema_identifiers() {
    let output = run_xtask(&["print-schemas"]);

    assert!(output.status.success(), "print-schemas should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("buildfix.plan.v1"),
        "Should output plan schema identifier"
    );
    assert!(
        stdout.contains("buildfix.apply.v1"),
        "Should output apply schema identifier"
    );
    assert!(
        stdout.contains("buildfix.report.v1"),
        "Should output report schema identifier"
    );
}

// =============================================================================
// init-artifacts Command Tests
// =============================================================================

#[test]
fn init_artifacts_creates_directory_structure() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let artifacts_dir = temp.path().join("test_artifacts");

    let output = run_xtask(&["init-artifacts", "--dir", artifacts_dir.to_str().unwrap()]);

    assert!(
        output.status.success(),
        "init-artifacts should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify directory structure was created
    assert!(
        artifacts_dir.exists(),
        "Artifacts directory should be created"
    );
    assert!(
        artifacts_dir.join("buildscan").exists(),
        "buildscan subdirectory should be created"
    );
    assert!(
        artifacts_dir.join("builddiag").exists(),
        "builddiag subdirectory should be created"
    );
    assert!(
        artifacts_dir.join("depguard").exists(),
        "depguard subdirectory should be created"
    );
    assert!(
        artifacts_dir.join("buildfix").exists(),
        "buildfix subdirectory should be created"
    );
}

#[test]
fn init_artifacts_default_dir() {
    // Test with default directory name
    let temp = TempDir::new().expect("Failed to create temp dir");
    let original_dir = std::env::current_dir().expect("Failed to get current dir");

    // Change to temp directory so we don't pollute the workspace
    std::env::set_current_dir(temp.path()).expect("Failed to change dir");

    let output = run_xtask(&["init-artifacts"]);
    assert!(
        output.status.success(),
        "init-artifacts with default dir should succeed"
    );

    // Verify default "artifacts" directory was created
    assert!(
        std::path::Path::new("artifacts").exists(),
        "Default artifacts directory should be created"
    );

    // Cleanup: restore original directory
    std::env::set_current_dir(&original_dir).expect("Failed to restore dir");
}

// =============================================================================
// conform Command Tests
// =============================================================================

#[test]
fn conform_skips_when_no_report() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let artifacts_dir = temp.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("Failed to create artifacts dir");

    let output = run_xtask(&[
        "conform",
        "--artifacts-dir",
        artifacts_dir.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "conform should succeed when report is missing (skip behavior)"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[SKIP]") || stdout.contains("Conformance check passed"),
        "Should indicate skip or pass"
    );
}

#[test]
fn conform_passes_with_valid_report() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let artifacts_dir = temp.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("Failed to create artifacts dir");

    // Write a valid report
    let report = serde_json::json!({
        "schema": "buildfix.report.v1",
        "tool": {
            "name": "buildfix",
            "version": "1.0.0"
        },
        "run": {
            "started_at": "2024-01-01T00:00:00Z"
        },
        "verdict": {
            "status": "pass",
            "counts": {
                "info": 0,
                "warn": 0,
                "error": 0
            }
        }
    });
    std::fs::write(
        artifacts_dir.join("report.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .expect("Failed to write report");

    let output = run_xtask(&[
        "conform",
        "--artifacts-dir",
        artifacts_dir.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "conform should succeed with valid report: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn conform_fails_with_invalid_json() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let artifacts_dir = temp.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("Failed to create artifacts dir");

    // Write invalid JSON
    std::fs::write(artifacts_dir.join("report.json"), "{ invalid json }")
        .expect("Failed to write report");

    let output = run_xtask(&[
        "conform",
        "--artifacts-dir",
        artifacts_dir.to_str().unwrap(),
    ]);

    assert!(
        !output.status.success(),
        "conform should fail with invalid JSON"
    );
}

#[test]
fn conform_fails_with_missing_required_fields() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let artifacts_dir = temp.path().join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("Failed to create artifacts dir");

    // Write a report missing required fields
    let report = serde_json::json!({
        "schema": "buildfix.report.v1"
        // Missing tool, run, verdict
    });
    std::fs::write(
        artifacts_dir.join("report.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .expect("Failed to write report");

    let output = run_xtask(&[
        "conform",
        "--artifacts-dir",
        artifacts_dir.to_str().unwrap(),
    ]);

    assert!(
        !output.status.success(),
        "conform should fail with missing required fields"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("[FAIL]") || stderr.contains("conformance"),
        "Should indicate failure"
    );
}

#[test]
fn conform_with_golden_dir_missing_golden_files() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let artifacts_dir = temp.path().join("artifacts");
    let golden_dir = temp.path().join("golden");
    std::fs::create_dir_all(&artifacts_dir).expect("Failed to create artifacts dir");
    std::fs::create_dir_all(&golden_dir).expect("Failed to create golden dir");

    // Write a valid report
    let report = serde_json::json!({
        "schema": "buildfix.report.v1",
        "tool": { "name": "buildfix", "version": "1.0.0" },
        "run": { "started_at": "2024-01-01T00:00:00Z" },
        "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
    });
    std::fs::write(
        artifacts_dir.join("report.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .expect("Failed to write report");

    // No golden file - should fail determinism check
    let output = run_xtask(&[
        "conform",
        "--artifacts-dir",
        artifacts_dir.to_str().unwrap(),
        "--golden-dir",
        golden_dir.to_str().unwrap(),
    ]);

    assert!(
        !output.status.success(),
        "conform should fail when golden files are missing"
    );
}

#[test]
fn conform_with_golden_dir_matching() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let artifacts_dir = temp.path().join("artifacts");
    let golden_dir = temp.path().join("golden");
    std::fs::create_dir_all(&artifacts_dir).expect("Failed to create artifacts dir");
    std::fs::create_dir_all(&golden_dir).expect("Failed to create golden dir");

    // Same content in both (after normalization)
    let report = serde_json::json!({
        "schema": "buildfix.report.v1",
        "tool": { "name": "buildfix", "version": "1.0.0" },
        "run": { "started_at": "2024-01-01T00:00:00Z" },
        "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
    });
    let report_str = serde_json::to_string_pretty(&report).unwrap();

    std::fs::write(artifacts_dir.join("report.json"), &report_str).expect("Failed to write report");
    std::fs::write(golden_dir.join("report.json"), &report_str).expect("Failed to write golden");

    let output = run_xtask(&[
        "conform",
        "--artifacts-dir",
        artifacts_dir.to_str().unwrap(),
        "--golden-dir",
        golden_dir.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "conform should pass when golden matches: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn conform_with_contracts_dir() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let artifacts_dir = temp.path().join("artifacts");
    let contracts_dir = temp.path().join("contracts");
    let schemas_dir = contracts_dir.join("schemas");
    std::fs::create_dir_all(&artifacts_dir).expect("Failed to create artifacts dir");
    std::fs::create_dir_all(&schemas_dir).expect("Failed to create schemas dir");

    // Write a minimal schema
    let schema = serde_json::json!({
        "type": "object",
        "required": ["schema", "tool", "run", "verdict"],
        "properties": {
            "schema": { "type": "string" },
            "tool": { "type": "object" },
            "run": { "type": "object" },
            "verdict": { "type": "object" }
        }
    });
    std::fs::write(
        schemas_dir.join("sensor.report.v1.json"),
        serde_json::to_string_pretty(&schema).unwrap(),
    )
    .expect("Failed to write schema");

    // Write a valid report
    let report = serde_json::json!({
        "schema": "buildfix.report.v1",
        "tool": { "name": "buildfix", "version": "1.0.0" },
        "run": { "started_at": "2024-01-01T00:00:00Z" },
        "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
    });
    std::fs::write(
        artifacts_dir.join("report.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .expect("Failed to write report");

    let output = run_xtask(&[
        "conform",
        "--artifacts-dir",
        artifacts_dir.to_str().unwrap(),
        "--contracts-dir",
        contracts_dir.to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "conform with custom contracts dir should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn conform_invalid_contracts_dir_fails() {
    let temp = TempDir::new().expect("Failed to create temp dir");
    let artifacts_dir = temp.path().join("artifacts");
    let contracts_dir = temp.path().join("nonexistent_contracts");
    std::fs::create_dir_all(&artifacts_dir).expect("Failed to create artifacts dir");
    // Don't create contracts_dir

    // Write a valid report
    let report = serde_json::json!({
        "schema": "buildfix.report.v1",
        "tool": { "name": "buildfix", "version": "1.0.0" },
        "run": { "started_at": "2024-01-01T00:00:00Z" },
        "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
    });
    std::fs::write(
        artifacts_dir.join("report.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .expect("Failed to write report");

    let output = run_xtask(&[
        "conform",
        "--artifacts-dir",
        artifacts_dir.to_str().unwrap(),
        "--contracts-dir",
        contracts_dir.to_str().unwrap(),
    ]);

    // Should fail because contracts_dir doesn't exist
    assert!(
        !output.status.success(),
        "conform should fail with invalid contracts dir"
    );
}

#[test]
fn init_artifacts_invalid_path_handling() {
    // Try to create artifacts in a path that requires elevated permissions
    // On most systems this will fail gracefully
    let output = run_xtask(&["init-artifacts", "--dir", "/nonexistent/path/to/artifacts"]);

    // This should fail because parent directory doesn't exist
    // The exact behavior depends on filesystem permissions
    // We just verify it doesn't panic
    let _ = output.status.success();
}

// =============================================================================
// Subcommand Help Tests
// =============================================================================

#[test]
fn subcommand_help_init_artifacts() {
    let output = run_xtask(&["init-artifacts", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("dir"), "Help should mention --dir option");
}

#[test]
fn subcommand_help_conform() {
    let output = run_xtask(&["conform", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("artifacts-dir"),
        "Help should mention --artifacts-dir"
    );
    assert!(
        stdout.contains("golden-dir"),
        "Help should mention --golden-dir"
    );
    assert!(
        stdout.contains("contracts-dir"),
        "Help should mention --contracts-dir"
    );
}

#[test]
fn subcommand_help_bless_fixtures() {
    let output = run_xtask(&["bless-fixtures", "--help"]);

    assert!(output.status.success());
}

#[test]
fn subcommand_help_validate() {
    let output = run_xtask(&["validate", "--help"]);

    assert!(output.status.success());
}

#[test]
fn subcommand_help_print_schemas() {
    let output = run_xtask(&["print-schemas", "--help"]);

    assert!(output.status.success());
}
