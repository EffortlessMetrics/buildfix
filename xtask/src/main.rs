use anyhow::Context;
use clap::{Parser, Subcommand};
use fs_err as fs;

use std::process::Command as ProcessCommand;

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Workspace helper tasks")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print schema identifiers used by buildfix.
    PrintSchemas,
    /// Create an empty artifacts layout (artifacts/<sensor>/report.json placeholders).
    InitArtifacts {
        #[arg(long, default_value = "artifacts")]
        dir: String,
    },
    /// Bless golden fixtures (overwrite expected outputs).
    BlessFixtures,
    /// Validate receipts and buildfix artifacts against schemas.
    Validate,
    /// Run conformance checks on buildfix output.
    Conform {
        /// Directory containing buildfix artifacts.
        #[arg(long, default_value = "artifacts/buildfix")]
        artifacts_dir: String,
        /// Directory containing golden files for determinism check.
        #[arg(long)]
        golden_dir: Option<String>,
        /// Directory containing cockpit contract schemas.
        /// Falls back to the embedded constant when omitted.
        #[arg(long, env = "COCKPIT_CONTRACTS_DIR")]
        contracts_dir: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.cmd {
        Command::PrintSchemas => {
            println!("{}", buildfix_types::schema::BUILDFIX_PLAN_V1);
            println!("{}", buildfix_types::schema::BUILDFIX_APPLY_V1);
            println!("{}", buildfix_types::schema::BUILDFIX_REPORT_V1);
        }
        Command::InitArtifacts { dir } => {
            fs::create_dir_all(&dir).with_context(|| format!("create {dir}"))?;
            for s in ["buildscan", "builddiag", "depguard", "buildfix"] {
                fs::create_dir_all(format!("{dir}/{s}"))?;
            }
            println!("initialized {dir}/{{buildscan,builddiag,depguard,buildfix}}");
        }
        Command::BlessFixtures => {
            let status = cargo_command()
                .args(["test", "-p", "buildfix-domain", "--test", "golden_fixtures"])
                .env("BUILDFIX_BLESS", "1")
                .status()
                .context("run golden fixture blessing")?;
            if !status.success() {
                anyhow::bail!("bless-fixtures failed");
            }
        }
        Command::Validate => {
            let status = cargo_command()
                .args(["run", "-p", "buildfix", "--", "validate"])
                .status()
                .context("run buildfix validate")?;
            if !status.success() {
                anyhow::bail!("validate failed");
            }
        }
        Command::Conform {
            artifacts_dir,
            golden_dir,
            contracts_dir,
        } => {
            cmd_conform(
                &artifacts_dir,
                golden_dir.as_deref(),
                contracts_dir.as_deref(),
            )?;
        }
    }
    Ok(())
}

fn cargo_command() -> ProcessCommand {
    let cmd = std::env::var("XTASK_CARGO").unwrap_or_else(|_| "cargo".to_string());
    ProcessCommand::new(cmd)
}

/// Schema for sensor.report.v1 (embedded from vendor).
const SENSOR_REPORT_V1_SCHEMA: &str =
    include_str!("../../vendor/cockpit-contracts/schemas/sensor.report.v1.json");

fn cmd_conform(
    artifacts_dir: &str,
    golden_dir: Option<&str>,
    contracts_dir: Option<&str>,
) -> anyhow::Result<()> {
    let mut failures: Vec<String> = Vec::new();

    // Load schema: prefer runtime contracts_dir, fall back to embedded constant.
    let schema_str: String;
    let sensor_schema = if let Some(dir) = contracts_dir {
        let schema_path = format!("{}/schemas/sensor.report.v1.json", dir);
        schema_str =
            fs::read_to_string(&schema_path).with_context(|| format!("read {}", schema_path))?;
        schema_str.as_str()
    } else {
        SENSOR_REPORT_V1_SCHEMA
    };

    // 1. Schema validation - check report.json against sensor.report.v1 schema
    let report_path = format!("{}/report.json", artifacts_dir);
    if std::path::Path::new(&report_path).exists() {
        match validate_against_schema(&report_path, sensor_schema) {
            Ok(()) => println!("[PASS] Schema validation: {}", report_path),
            Err(errors) => {
                println!("[FAIL] Schema validation: {}", report_path);
                for err in &errors {
                    println!("  - {}", err);
                }
                failures.extend(
                    errors
                        .into_iter()
                        .map(|e| format!("{}: {}", report_path, e)),
                );
            }
        }
    } else {
        println!("[SKIP] Schema validation: {} (not found)", report_path);
    }

    // 2. Required fields check - verify required fields even on tool error
    if std::path::Path::new(&report_path).exists() {
        match check_required_fields(&report_path) {
            Ok(()) => println!("[PASS] Required fields: {}", report_path),
            Err(errors) => {
                println!("[FAIL] Required fields: {}", report_path);
                for err in &errors {
                    println!("  - {}", err);
                }
                failures.extend(
                    errors
                        .into_iter()
                        .map(|e| format!("{}: {}", report_path, e)),
                );
            }
        }
    }

    // 3. Determinism check - compare against golden files (if provided)
    if let Some(golden) = golden_dir {
        match check_determinism(artifacts_dir, golden) {
            Ok(()) => println!("[PASS] Determinism check"),
            Err(errors) => {
                println!("[FAIL] Determinism check");
                for err in &errors {
                    println!("  - {}", err);
                }
                failures.extend(errors);
            }
        }
    } else {
        println!("[SKIP] Determinism check: no golden_dir provided");
    }

    if !failures.is_empty() {
        println!("\n{} conformance failures:", failures.len());
        for f in &failures {
            println!("  - {}", f);
        }
        anyhow::bail!("conformance check failed");
    }

    println!("\nConformance check passed.");
    Ok(())
}

fn validate_against_schema(file_path: &str, schema_str: &str) -> Result<(), Vec<String>> {
    let contents = fs::read_to_string(file_path).map_err(|e| vec![e.to_string()])?;
    let json: serde_json::Value =
        serde_json::from_str(&contents).map_err(|e| vec![format!("JSON parse: {}", e)])?;

    let schema_json: serde_json::Value =
        serde_json::from_str(schema_str).map_err(|e| vec![format!("Schema parse: {}", e)])?;

    let compiled = jsonschema::draft202012::new(&schema_json)
        .map_err(|e| vec![format!("Schema compile: {}", e)])?;

    let errors: Vec<String> = compiled.iter_errors(&json).map(|e| e.to_string()).collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_required_fields(file_path: &str) -> Result<(), Vec<String>> {
    let contents = fs::read_to_string(file_path).map_err(|e| vec![e.to_string()])?;
    let json: serde_json::Value =
        serde_json::from_str(&contents).map_err(|e| vec![format!("JSON parse: {}", e)])?;

    let mut errors = Vec::new();

    // Check for required top-level fields
    let required = ["schema", "tool", "run", "verdict"];
    for field in required {
        if json.get(field).is_none() {
            errors.push(format!("missing required field: {}", field));
        }
    }

    // Check tool.name and tool.version
    if let Some(tool) = json.get("tool") {
        if tool.get("name").is_none() {
            errors.push("missing required field: tool.name".to_string());
        }
        if tool.get("version").is_none() {
            errors.push("missing required field: tool.version".to_string());
        }
    }

    // Check run.started_at
    if let Some(run) = json.get("run")
        && run.get("started_at").is_none()
    {
        errors.push("missing required field: run.started_at".to_string());
    }

    // Check verdict.status and verdict.counts
    if let Some(verdict) = json.get("verdict") {
        if verdict.get("status").is_none() {
            errors.push("missing required field: verdict.status".to_string());
        }
        if verdict.get("counts").is_none() {
            errors.push("missing required field: verdict.counts".to_string());
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_determinism(artifacts_dir: &str, golden_dir: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    for file in ["plan.json", "apply.json", "report.json"] {
        let actual_path = format!("{}/{}", artifacts_dir, file);
        let golden_path = format!("{}/{}", golden_dir, file);

        if !std::path::Path::new(&actual_path).exists() {
            continue;
        }
        if !std::path::Path::new(&golden_path).exists() {
            errors.push(format!("golden file missing: {}", golden_path));
            continue;
        }

        let actual = fs::read_to_string(&actual_path).map_err(|e| vec![e.to_string()])?;
        let golden = fs::read_to_string(&golden_path).map_err(|e| vec![e.to_string()])?;

        // Parse and re-serialize to normalize, stripping volatile fields
        let actual_normalized = normalize_for_determinism(&actual)?;
        let golden_normalized = normalize_for_determinism(&golden)?;

        if actual_normalized != golden_normalized {
            errors.push(format!("{} differs from golden", file));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn normalize_for_determinism(json_str: &str) -> Result<String, Vec<String>> {
    let mut json: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| vec![format!("JSON parse: {}", e)])?;

    // Strip volatile fields that change between runs
    if let Some(obj) = json.as_object_mut() {
        // Strip run.started_at, run.ended_at, run.duration_ms
        if let Some(run) = obj.get_mut("run").and_then(|r| r.as_object_mut()) {
            run.remove("started_at");
            run.remove("ended_at");
            run.remove("duration_ms");
        }
    }

    serde_json::to_string_pretty(&json).map_err(|e| vec![e.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs_err as fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn normalize_for_determinism_strips_run_fields() {
        let input = serde_json::json!({
            "schema": "buildfix.report.v1",
            "tool": { "name": "buildfix", "version": "1.0" },
            "run": { "started_at": "t0", "ended_at": "t1", "duration_ms": 5 },
            "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
        })
        .to_string();

        let normalized = normalize_for_determinism(&input).expect("normalize");
        let value: serde_json::Value = serde_json::from_str(&normalized).expect("parse normalized");
        let run = value.get("run").and_then(|v| v.as_object()).expect("run");
        assert!(run.get("started_at").is_none());
        assert!(run.get("ended_at").is_none());
        assert!(run.get("duration_ms").is_none());
    }

    #[test]
    fn check_required_fields_reports_missing() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("report.json");
        fs::write(
            &path,
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix" },
                "run": {},
                "verdict": {}
            }"#,
        )
        .expect("write");

        let errors = check_required_fields(path.to_str().unwrap()).expect_err("missing fields");
        assert!(errors.iter().any(|e| e.contains("tool.version")));
        assert!(errors.iter().any(|e| e.contains("run.started_at")));
        assert!(errors.iter().any(|e| e.contains("verdict.status")));
        assert!(errors.iter().any(|e| e.contains("verdict.counts")));
    }

    #[test]
    fn check_required_fields_accepts_minimal_valid_payload() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("report.json");
        fs::write(
            &path,
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix", "version": "1.0" },
                "run": { "started_at": "t0" },
                "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
            }"#,
        )
        .expect("write");

        check_required_fields(path.to_str().unwrap()).expect("valid");
    }

    #[test]
    fn check_required_fields_reports_missing_tool_name() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("report.json");
        fs::write(
            &path,
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "version": "1.0" },
                "run": { "started_at": "t0" },
                "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
            }"#,
        )
        .expect("write");

        let errors = check_required_fields(path.to_str().unwrap()).expect_err("missing tool.name");
        assert!(errors.iter().any(|e| e.contains("tool.name")));
    }

    fn write_fake_cargo(dir: &TempDir, exit_code: i32) {
        let script = dir.path().join("cargo.cmd");
        let contents = format!("@echo off\r\nexit /b {}\r\n", exit_code);
        fs::write(&script, contents).expect("write cargo stub");
    }

    fn with_fake_cargo(exit_code: i32, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let temp = TempDir::new().expect("temp dir");
        write_fake_cargo(&temp, exit_code);

        let old = std::env::var_os("XTASK_CARGO");
        unsafe {
            std::env::set_var("XTASK_CARGO", temp.path().join("cargo.cmd"));
        }

        f();

        match old {
            Some(value) => unsafe {
                std::env::set_var("XTASK_CARGO", value);
            },
            None => unsafe {
                std::env::remove_var("XTASK_CARGO");
            },
        }
    }

    fn with_fake_cargo_existing(exit_code: i32, existing: &str, f: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().expect("env lock");
        unsafe {
            std::env::set_var("XTASK_CARGO", existing);
        }
        let temp = TempDir::new().expect("temp dir");
        write_fake_cargo(&temp, exit_code);

        let old = std::env::var_os("XTASK_CARGO");
        unsafe {
            std::env::set_var("XTASK_CARGO", temp.path().join("cargo.cmd"));
        }

        f();

        match old {
            Some(value) => unsafe {
                std::env::set_var("XTASK_CARGO", value);
            },
            None => unsafe {
                std::env::remove_var("XTASK_CARGO");
            },
        }
    }

    #[test]
    fn validate_against_schema_reports_errors() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("payload.json");
        fs::write(&path, r#"{"name": 123}"#).expect("write");

        let schema = r#"{
            "type": "object",
            "required": ["name"],
            "properties": { "name": { "type": "string" } }
        }"#;

        let errors = validate_against_schema(path.to_str().unwrap(), schema).expect_err("errors");
        assert!(!errors.is_empty());
    }

    #[test]
    fn check_determinism_passes_after_normalization() {
        let temp = TempDir::new().expect("temp dir");
        let artifacts = temp.path().join("artifacts");
        let golden = temp.path().join("golden");
        fs::create_dir_all(&artifacts).expect("artifacts");
        fs::create_dir_all(&golden).expect("golden");

        let actual = r#"{
            "schema": "buildfix.report.v1",
            "tool": { "name": "buildfix", "version": "1.0" },
            "run": { "started_at": "2020-01-01T00:00:00Z", "ended_at": "2020-01-01T00:00:01Z" },
            "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
        }"#;

        let golden_content = r#"{
            "schema": "buildfix.report.v1",
            "tool": { "name": "buildfix", "version": "1.0" },
            "run": { "started_at": "2021-01-01T00:00:00Z", "ended_at": "2021-01-01T00:00:01Z" },
            "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
        }"#;

        fs::write(artifacts.join("report.json"), actual).expect("write actual");
        fs::write(golden.join("report.json"), golden_content).expect("write golden");

        check_determinism(artifacts.to_str().unwrap(), golden.to_str().unwrap())
            .expect("determinism");
    }

    #[test]
    fn check_determinism_reports_missing_golden() {
        let temp = TempDir::new().expect("temp dir");
        let artifacts = temp.path().join("artifacts");
        let golden = temp.path().join("golden");
        fs::create_dir_all(&artifacts).expect("artifacts");
        fs::create_dir_all(&golden).expect("golden");

        fs::write(
            artifacts.join("report.json"),
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix", "version": "1.0" },
                "run": { "started_at": "2020-01-01T00:00:00Z" },
                "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
            }"#,
        )
        .expect("write actual");

        let errors = check_determinism(artifacts.to_str().unwrap(), golden.to_str().unwrap())
            .expect_err("missing golden");
        assert!(errors.iter().any(|e| e.contains("golden file missing")));
    }

    #[test]
    fn check_determinism_reports_mismatch() {
        let temp = TempDir::new().expect("temp dir");
        let artifacts = temp.path().join("artifacts");
        let golden = temp.path().join("golden");
        fs::create_dir_all(&artifacts).expect("artifacts");
        fs::create_dir_all(&golden).expect("golden");

        fs::write(
            artifacts.join("report.json"),
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix", "version": "1.0" },
                "run": { "started_at": "2020-01-01T00:00:00Z" },
                "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
            }"#,
        )
        .expect("write actual");

        fs::write(
            golden.join("report.json"),
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix", "version": "1.0" },
                "run": { "started_at": "2020-01-01T00:00:00Z" },
                "verdict": { "status": "fail", "counts": { "info": 0, "warn": 0, "error": 1 } }
            }"#,
        )
        .expect("write golden");

        let errors = check_determinism(artifacts.to_str().unwrap(), golden.to_str().unwrap())
            .expect_err("mismatch");
        assert!(errors.iter().any(|e| e.contains("differs from golden")));
    }

    #[test]
    fn cmd_conform_passes_with_valid_report() {
        let temp = TempDir::new().expect("temp dir");
        let artifacts = temp.path().join("artifacts");
        fs::create_dir_all(&artifacts).expect("artifacts");

        fs::write(
            artifacts.join("report.json"),
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix", "version": "1.0" },
                "run": { "started_at": "2020-01-01T00:00:00Z" },
                "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
            }"#,
        )
        .expect("write report");

        cmd_conform(artifacts.to_str().unwrap(), None, None).expect("conform");
    }

    #[test]
    fn cmd_conform_skips_when_report_missing() {
        let temp = TempDir::new().expect("temp dir");
        let artifacts = temp.path().join("artifacts");
        fs::create_dir_all(&artifacts).expect("artifacts");

        cmd_conform(artifacts.to_str().unwrap(), None, None).expect("conform");
    }

    #[test]
    fn cmd_conform_reports_determinism_failure() {
        let temp = TempDir::new().expect("temp dir");
        let artifacts = temp.path().join("artifacts");
        let golden = temp.path().join("golden");
        fs::create_dir_all(&artifacts).expect("artifacts");
        fs::create_dir_all(&golden).expect("golden");

        fs::write(
            artifacts.join("report.json"),
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix", "version": "1.0" },
                "run": { "started_at": "2020-01-01T00:00:00Z" },
                "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
            }"#,
        )
        .expect("write report");

        fs::write(
            golden.join("report.json"),
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix", "version": "1.0" },
                "run": { "started_at": "2020-01-01T00:00:00Z" },
                "verdict": { "status": "fail", "counts": { "info": 0, "warn": 0, "error": 1 } }
            }"#,
        )
        .expect("write golden");

        let err = cmd_conform(
            artifacts.to_str().unwrap(),
            Some(golden.to_str().unwrap()),
            None,
        )
        .expect_err("determinism failure");
        assert!(err.to_string().contains("conformance check failed"));
    }

    #[test]
    fn cmd_conform_reads_schema_from_contracts_dir() {
        let temp = TempDir::new().expect("temp dir");
        let artifacts = temp.path().join("artifacts");
        let contracts = temp.path().join("contracts");
        fs::create_dir_all(contracts.join("schemas")).expect("schemas");
        fs::create_dir_all(&artifacts).expect("artifacts");

        fs::write(
            contracts.join("schemas").join("sensor.report.v1.json"),
            r#"{
                "type": "object",
                "required": ["schema", "tool", "run", "verdict"],
                "properties": {
                    "schema": { "type": "string" },
                    "tool": { "type": "object" },
                    "run": { "type": "object" },
                    "verdict": { "type": "object" }
                }
            }"#,
        )
        .expect("write schema");

        fs::write(
            artifacts.join("report.json"),
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix", "version": "1.0" },
                "run": { "started_at": "2020-01-01T00:00:00Z" },
                "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
            }"#,
        )
        .expect("write report");

        cmd_conform(
            artifacts.to_str().unwrap(),
            None,
            Some(contracts.to_str().unwrap()),
        )
        .expect("conform");
    }

    #[test]
    fn cmd_conform_fails_on_invalid_report() {
        let temp = TempDir::new().expect("temp dir");
        let artifacts = temp.path().join("artifacts");
        fs::create_dir_all(&artifacts).expect("artifacts");
        fs::write(artifacts.join("report.json"), "{}").expect("write report");

        let err = cmd_conform(artifacts.to_str().unwrap(), None, None).expect_err("fail");
        assert!(err.to_string().contains("conformance check failed"));
    }

    #[test]
    fn run_init_artifacts_creates_layout() {
        let temp = TempDir::new().expect("temp dir");
        let dir = temp.path().join("artifacts");
        run(Cli {
            cmd: Command::InitArtifacts {
                dir: dir.to_string_lossy().to_string(),
            },
        })
        .expect("init artifacts");

        for s in ["buildscan", "builddiag", "depguard", "buildfix"] {
            assert!(dir.join(s).exists());
        }
    }

    #[test]
    fn run_print_schemas_executes() {
        run(Cli {
            cmd: Command::PrintSchemas,
        })
        .expect("print schemas");
    }

    #[test]
    fn run_conform_executes() {
        let temp = TempDir::new().expect("temp dir");
        let artifacts = temp.path().join("artifacts");
        fs::create_dir_all(&artifacts).expect("artifacts");

        fs::write(
            artifacts.join("report.json"),
            r#"{
                "schema": "buildfix.report.v1",
                "tool": { "name": "buildfix", "version": "1.0" },
                "run": { "started_at": "2020-01-01T00:00:00Z" },
                "verdict": { "status": "pass", "counts": { "info": 0, "warn": 0, "error": 0 } }
            }"#,
        )
        .expect("write report");

        run(Cli {
            cmd: Command::Conform {
                artifacts_dir: artifacts.to_string_lossy().to_string(),
                golden_dir: None,
                contracts_dir: None,
            },
        })
        .expect("conform");
    }

    #[test]
    fn with_fake_cargo_restores_existing_value() {
        with_fake_cargo_existing(0, "existing", || {});
        let restored = std::env::var("XTASK_CARGO").expect("restored");
        assert_eq!(restored, "existing");
    }

    #[test]
    fn run_bless_fixtures_success_and_failure() {
        with_fake_cargo(0, || {
            run(Cli {
                cmd: Command::BlessFixtures,
            })
            .expect("bless ok");
        });

        with_fake_cargo(1, || {
            let err = run(Cli {
                cmd: Command::BlessFixtures,
            })
            .expect_err("bless fails");
            assert!(err.to_string().contains("bless-fixtures failed"));
        });
    }

    #[test]
    fn run_validate_success_and_failure() {
        with_fake_cargo(0, || {
            run(Cli {
                cmd: Command::Validate,
            })
            .expect("validate ok");
        });

        with_fake_cargo(1, || {
            let err = run(Cli {
                cmd: Command::Validate,
            })
            .expect_err("validate fails");
            assert!(err.to_string().contains("validate failed"));
        });
    }
}
