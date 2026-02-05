use anyhow::Context;
use clap::{Parser, Subcommand};
use fs_err as fs;
use jsonschema::JSONSchema;
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
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
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
            let status = ProcessCommand::new("cargo")
                .args(["test", "-p", "buildfix-domain", "--test", "golden_fixtures"])
                .env("BUILDFIX_BLESS", "1")
                .status()
                .context("run golden fixture blessing")?;
            if !status.success() {
                anyhow::bail!("bless-fixtures failed");
            }
        }
        Command::Validate => {
            let status = ProcessCommand::new("cargo")
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
        } => {
            cmd_conform(&artifacts_dir, golden_dir.as_deref())?;
        }
    }
    Ok(())
}

/// Schema for sensor.report.v1 (embedded from contracts).
const SENSOR_REPORT_V1_SCHEMA: &str = include_str!("../../contracts/schemas/sensor.report.v1.json");

fn cmd_conform(artifacts_dir: &str, golden_dir: Option<&str>) -> anyhow::Result<()> {
    let mut failures: Vec<String> = Vec::new();

    // 1. Schema validation - check report.json against sensor.report.v1 schema
    let report_path = format!("{}/report.json", artifacts_dir);
    if std::path::Path::new(&report_path).exists() {
        match validate_against_schema(&report_path, SENSOR_REPORT_V1_SCHEMA) {
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

    let compiled = JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema_json)
        .map_err(|e| vec![format!("Schema compile: {}", e)])?;

    let result = compiled.validate(&json);
    if let Err(errors) = result {
        let msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
        if msgs.is_empty() {
            Ok(())
        } else {
            Err(msgs)
        }
    } else {
        Ok(())
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
    if let Some(run) = json.get("run") {
        if run.get("started_at").is_none() {
            errors.push("missing required field: run.started_at".to_string());
        }
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
