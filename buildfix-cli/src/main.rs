mod config;
mod explain;

use anyhow::Context;
use buildfix_core::adapters::{FsReceiptSource, FsWritePort, ShellGitPort};
use buildfix_core::pipeline::{run_apply, run_plan, write_apply_artifacts, write_plan_artifacts};
use buildfix_core::settings::{ApplySettings, PlanSettings, RunMode};
use buildfix_types::receipt::ToolInfo;
use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use config::{ConfigMerger, parse_cli_params};
use fs_err as fs;

use std::process::ExitCode;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

const PLAN_SCHEMA: &str = include_str!("../../schemas/buildfix.plan.v1.json");
const APPLY_SCHEMA: &str = include_str!("../../schemas/buildfix.apply.v1.json");
/// Canonical report is validated against the sensor envelope schema.
const REPORT_SCHEMA: &str =
    include_str!("../../vendor/cockpit-contracts/schemas/sensor.report.v1.json");
/// Buildfix-specific extras report is validated against the buildfix schema.
const BUILDFIX_REPORT_SCHEMA: &str = include_str!("../../schemas/buildfix.report.v1.json");

#[derive(Debug, Parser)]
#[command(
    name = "buildfix",
    version,
    about = "Receipt-driven repair tool for Cargo workspace hygiene."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate a deterministic fix plan from receipts.
    Plan(PlanArgs),
    /// Apply an existing plan (default: dry-run).
    Apply(ApplyArgs),
    /// Explain what a fix does, its safety rationale, and remediation guidance.
    Explain(ExplainArgs),
    /// List all available fixes with their safety classifications.
    ListFixes(ListFixesArgs),
    /// Validate receipts and buildfix artifacts against schemas.
    Validate(ValidateArgs),
}

#[derive(Debug, Parser)]
struct PlanArgs {
    /// Repository root (default: current directory).
    #[arg(long, default_value = ".")]
    repo_root: Utf8PathBuf,

    /// Artifacts directory containing receipts (default: <repo_root>/artifacts).
    #[arg(long)]
    artifacts_dir: Option<Utf8PathBuf>,

    /// Output directory for buildfix artifacts (default: <repo_root>/artifacts/buildfix).
    #[arg(long)]
    out_dir: Option<Utf8PathBuf>,

    /// Allowlist patterns for policy keys (apply-time).
    #[arg(long)]
    allow: Vec<String>,

    /// Denylist patterns for policy keys (apply-time).
    #[arg(long)]
    deny: Vec<String>,

    /// Disable sha256 preconditions (not recommended).
    #[arg(long, default_value_t = false)]
    no_clean_hashes: bool,

    /// Maximum number of operations allowed in the plan.
    #[arg(long)]
    max_ops: Option<u64>,

    /// Maximum number of files allowed to be modified.
    #[arg(long)]
    max_files: Option<u64>,

    /// Maximum size of the patch in bytes.
    #[arg(long)]
    max_patch_bytes: Option<u64>,

    /// Require git HEAD SHA precondition for each fix.
    /// Ensures plan can only be applied to the exact commit it was generated from.
    #[arg(long, default_value_t = false)]
    git_head_precondition: bool,

    /// Parameters for unsafe fixes (repeatable: key=value).
    #[arg(long)]
    param: Vec<String>,

    /// Run mode. In cockpit mode, policy blocks (exit 2) are mapped to exit 0.
    #[arg(long, value_enum, default_value = "standalone")]
    mode: CliRunMode,
}

#[derive(Debug, Parser)]
struct ApplyArgs {
    /// Repository root (default: current directory).
    #[arg(long, default_value = ".")]
    repo_root: Utf8PathBuf,

    /// Directory containing plan.json (default: <repo_root>/artifacts/buildfix).
    #[arg(long)]
    out_dir: Option<Utf8PathBuf>,

    /// Apply changes to disk. If omitted, runs a dry-run and only emits artifacts.
    #[arg(long, default_value_t = false)]
    apply: bool,

    /// Allow guarded fixes to run.
    #[arg(long, default_value_t = false)]
    allow_guarded: bool,

    /// Allow unsafe fixes to run.
    #[arg(long, default_value_t = false)]
    allow_unsafe: bool,

    /// Allow applying fixes when the git working tree has uncommitted changes.
    #[arg(long, default_value_t = false)]
    allow_dirty: bool,

    /// Parameters for unsafe fixes (repeatable: key=value).
    #[arg(long)]
    param: Vec<String>,

    /// Run mode. In cockpit mode, policy blocks (exit 2) are mapped to exit 0.
    #[arg(long, value_enum, default_value = "standalone")]
    mode: CliRunMode,
}

#[derive(Debug, Parser)]
struct ExplainArgs {
    /// Fix key or fix ID to explain (e.g., "resolver-v2", "path-dep-version").
    fix_key: String,
}

#[derive(Debug, Parser)]
struct ListFixesArgs {
    /// Output format (text, json).
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
}

#[derive(Debug, Parser)]
struct ValidateArgs {
    /// Repository root (default: current directory).
    #[arg(long, default_value = ".")]
    repo_root: Utf8PathBuf,

    /// Artifacts directory containing receipts (default: <repo_root>/artifacts).
    #[arg(long)]
    artifacts_dir: Option<Utf8PathBuf>,

    /// Output directory for buildfix artifacts (default: <repo_root>/artifacts/buildfix).
    #[arg(long)]
    out_dir: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
enum CliRunMode {
    #[default]
    Standalone,
    Cockpit,
}

impl From<CliRunMode> for RunMode {
    fn from(m: CliRunMode) -> Self {
        match m {
            CliRunMode::Standalone => RunMode::Standalone,
            CliRunMode::Cockpit => RunMode::Cockpit,
        }
    }
}

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => code,
        Err(e) => {
            error!("{:?}", e);
            ExitCode::from(1)
        }
    }
}

fn real_main() -> anyhow::Result<ExitCode> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Command::Plan(args) => cmd_plan(args),
        Command::Apply(args) => cmd_apply(args),
        Command::Explain(args) => {
            cmd_explain(args)?;
            Ok(ExitCode::from(0))
        }
        Command::ListFixes(args) => {
            cmd_list_fixes(args)?;
            Ok(ExitCode::from(0))
        }
        Command::Validate(args) => cmd_validate(args),
    }
}

fn cmd_plan(args: PlanArgs) -> anyhow::Result<ExitCode> {
    let repo_root = args.repo_root;
    let artifacts_dir = args
        .artifacts_dir
        .unwrap_or_else(|| repo_root.join("artifacts"));
    let out_dir = args
        .out_dir
        .unwrap_or_else(|| artifacts_dir.join("buildfix"));

    let cli_params = parse_cli_params(&args.param)?;

    // Load config file and merge with CLI arguments.
    let file_config = config::load_or_default(&repo_root).context("load buildfix.toml config")?;
    let merged = ConfigMerger::new(file_config).merge_plan_args(
        &args.allow,
        &args.deny,
        args.no_clean_hashes,
        &cli_params,
    );

    debug!(
        "merged config: allow={:?}, deny={:?}, require_clean_hashes={}, params={:?}",
        merged.allow, merged.deny, merged.require_clean_hashes, merged.params
    );

    let mode: RunMode = args.mode.into();

    let settings = PlanSettings {
        repo_root: repo_root.clone(),
        artifacts_dir: artifacts_dir.clone(),
        out_dir: out_dir.clone(),
        allow: merged.allow.clone(),
        deny: merged.deny.clone(),
        allow_guarded: merged.allow_guarded,
        allow_unsafe: merged.allow_unsafe,
        allow_dirty: merged.allow_dirty,
        max_ops: args.max_ops.or(merged.max_ops),
        max_files: args.max_files.or(merged.max_files),
        max_patch_bytes: args.max_patch_bytes.or(merged.max_patch_bytes),
        params: merged.params.clone(),
        require_clean_hashes: merged.require_clean_hashes,
        git_head_precondition: args.git_head_precondition,
        backup_suffix: merged.backups.suffix.clone(),
        mode,
    };

    let receipts_port = FsReceiptSource::new(artifacts_dir);
    let git = ShellGitPort;
    let writer = FsWritePort;
    let tool = tool_info();

    let outcome = run_plan(&settings, &receipts_port, &git, tool).map_err(|e| match e {
        buildfix_core::pipeline::ToolError::Internal(e) => e,
        buildfix_core::pipeline::ToolError::PolicyBlock => {
            anyhow::anyhow!("policy block")
        }
    })?;

    write_plan_artifacts(&outcome, &out_dir, &writer)?;

    info!("wrote plan to {}", out_dir);

    Ok(if outcome.policy_block && mode != RunMode::Cockpit {
        ExitCode::from(2)
    } else {
        ExitCode::from(0)
    })
}

fn cmd_apply(args: ApplyArgs) -> anyhow::Result<ExitCode> {
    let repo_root = args.repo_root;
    let out_dir = args
        .out_dir
        .unwrap_or_else(|| repo_root.join("artifacts").join("buildfix"));

    let cli_params = parse_cli_params(&args.param)?;

    // Load config file and merge with CLI arguments.
    let file_config = config::load_or_default(&repo_root).context("load buildfix.toml config")?;
    let merged = ConfigMerger::new(file_config).merge_apply_args(
        args.allow_guarded,
        args.allow_unsafe,
        &cli_params,
    );

    let allow_dirty = args.allow_dirty || merged.allow_dirty;
    let mode: RunMode = args.mode.into();

    debug!(
        "merged config: allow_guarded={}, allow_unsafe={}, allow_dirty={}",
        merged.allow_guarded, merged.allow_unsafe, allow_dirty
    );

    let settings = ApplySettings {
        repo_root: repo_root.clone(),
        out_dir: out_dir.clone(),
        dry_run: !args.apply,
        allow_guarded: merged.allow_guarded,
        allow_unsafe: merged.allow_unsafe,
        allow_dirty,
        params: merged.params.clone(),
        backup_enabled: merged.backups.enabled,
        backup_suffix: merged.backups.suffix.clone(),
        mode,
    };

    let git = ShellGitPort;
    let writer = FsWritePort;
    let tool = tool_info();

    let outcome = run_apply(&settings, &git, tool).map_err(|e| match e {
        buildfix_core::pipeline::ToolError::Internal(e) => e,
        buildfix_core::pipeline::ToolError::PolicyBlock => {
            anyhow::anyhow!("policy block")
        }
    })?;

    write_apply_artifacts(&outcome, &out_dir, &writer)?;

    info!("wrote apply artifacts to {}", out_dir);

    Ok(if outcome.policy_block && mode != RunMode::Cockpit {
        ExitCode::from(2)
    } else {
        ExitCode::from(0)
    })
}

fn cmd_validate(args: ValidateArgs) -> anyhow::Result<ExitCode> {
    let repo_root = args.repo_root;
    let artifacts_dir = args
        .artifacts_dir
        .unwrap_or_else(|| repo_root.join("artifacts"));
    let out_dir = args
        .out_dir
        .unwrap_or_else(|| artifacts_dir.join("buildfix"));

    let receipts = buildfix_receipts::load_receipts(&artifacts_dir)
        .with_context(|| format!("load receipts from {}", artifacts_dir))?;
    let mut policy_failures = Vec::new();
    for r in &receipts {
        if let Err(e) = &r.receipt {
            policy_failures.push(format!("{}: {}", r.path, e));
        }
    }

    for (path, schema) in [
        (out_dir.join("plan.json"), PLAN_SCHEMA),
        (out_dir.join("apply.json"), APPLY_SCHEMA),
        (out_dir.join("report.json"), REPORT_SCHEMA),
        (
            out_dir.join("extras").join("buildfix.report.v1.json"),
            BUILDFIX_REPORT_SCHEMA,
        ),
    ] {
        match validate_file_if_exists(&path, schema)? {
            ValidateOutcome::Missing | ValidateOutcome::Ok => {}
            ValidateOutcome::SchemaErrors(errors) => {
                for err in errors {
                    policy_failures.push(format!("{}: {}", path, err));
                }
            }
        }
    }

    if !policy_failures.is_empty() {
        for msg in &policy_failures {
            error!("{}", msg);
        }
        return Ok(ExitCode::from(2));
    }

    info!("validation successful");
    Ok(ExitCode::from(0))
}

enum ValidateOutcome {
    Missing,
    Ok,
    SchemaErrors(Vec<String>),
}

fn validate_file_if_exists(path: &Utf8Path, schema_str: &str) -> anyhow::Result<ValidateOutcome> {
    if !path.exists() {
        return Ok(ValidateOutcome::Missing);
    }
    let contents = fs::read_to_string(path).with_context(|| format!("read {}", path))?;
    let json: serde_json::Value = serde_json::from_str(&contents).context("parse json")?;

    let schema_json: serde_json::Value =
        serde_json::from_str(schema_str).context("parse schema")?;
    let compiled = jsonschema::draft202012::new(&schema_json)
        .map_err(|e| anyhow::anyhow!("compile schema: {}", e))?;
    let errors: Vec<String> = compiled.iter_errors(&json).map(|e| e.to_string()).collect();
    if !errors.is_empty() {
        return Ok(ValidateOutcome::SchemaErrors(errors));
    }
    Ok(ValidateOutcome::Ok)
}

fn tool_info() -> ToolInfo {
    ToolInfo {
        name: "buildfix".to_string(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        repo: None,
        commit: None,
    }
}

fn cmd_explain(args: ExplainArgs) -> anyhow::Result<()> {
    use explain::{
        format_safety_class, list_fix_keys, lookup_fix, policy_keys, safety_class_meaning,
    };

    let Some(fix) = lookup_fix(&args.fix_key) else {
        let available = list_fix_keys().join(", ");
        anyhow::bail!(
            "Unknown fix key: '{}'\n\nAvailable fixes: {}",
            args.fix_key,
            available
        );
    };

    // Title and basic info
    println!("================================================================================");
    println!("FIX: {}", fix.title);
    println!("================================================================================");
    println!();
    println!("Key:     {}", fix.key);
    println!("Fix ID:  {}", fix.fix_id);
    println!("Policy:  {}", policy_keys(fix).join(", "));
    println!("Safety:  {}", format_safety_class(fix.safety));
    println!();

    // Description
    println!("DESCRIPTION");
    println!("--------------------------------------------------------------------------------");
    println!("{}", fix.description);
    println!();

    // Triggering findings
    println!("TRIGGERING FINDINGS");
    println!("--------------------------------------------------------------------------------");
    println!("This fix is triggered by sensor findings matching:");
    println!();
    for trigger in fix.triggers {
        let code_part = trigger
            .code
            .map(|c| format!(" / {}", c))
            .unwrap_or_default();
        println!("  - {} / {}{}", trigger.sensor, trigger.check_id, code_part);
    }
    println!();

    // Safety class explanation
    println!("SAFETY CLASS: {}", format_safety_class(fix.safety));
    println!("--------------------------------------------------------------------------------");
    println!("{}", safety_class_meaning(fix.safety));
    println!();

    // Safety rationale
    println!("SAFETY RATIONALE");
    println!("--------------------------------------------------------------------------------");
    println!("{}", fix.safety_rationale);
    println!();

    // Remediation guidance
    println!("REMEDIATION GUIDANCE");
    println!("--------------------------------------------------------------------------------");
    println!("{}", fix.remediation);
    println!();

    Ok(())
}

fn cmd_list_fixes(args: ListFixesArgs) -> anyhow::Result<()> {
    use explain::{FIX_REGISTRY, format_safety_class, policy_keys};

    match args.format {
        OutputFormat::Text => {
            println!("Available fixes:\n");
            println!("  {:<24} {:<10} TITLE", "KEY", "SAFETY");
            println!("  {:<24} {:<10} -----", "---", "------");
            for fix in FIX_REGISTRY {
                let policy = policy_keys(fix).join(", ");
                println!(
                    "  {:<24} {:<10} {}",
                    fix.key,
                    format_safety_class(fix.safety),
                    fix.title
                );
                println!("    policy: {}", policy);
            }
            println!();
            println!("Use 'buildfix explain <key>' for details.");
        }
        OutputFormat::Json => {
            let fixes: Vec<_> = FIX_REGISTRY
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "key": f.key,
                        "fix_id": f.fix_id,
                        "title": f.title,
                        "safety": format_safety_class(f.safety).to_lowercase(),
                        "policy_keys": policy_keys(f),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&fixes)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_file_if_exists, ValidateOutcome};
    use camino::Utf8PathBuf;
    use tempfile::TempDir;

    #[test]
    fn validate_file_if_exists_missing_returns_missing() {
        let temp = TempDir::new().expect("temp dir");
        let path = Utf8PathBuf::from_path_buf(temp.path().join("missing.json")).expect("utf8");
        let outcome = validate_file_if_exists(&path, r#"{"type": "object"}"#).expect("ok");
        assert!(matches!(outcome, ValidateOutcome::Missing));
    }

    #[test]
    fn validate_file_if_exists_reports_schema_errors() {
        let temp = TempDir::new().expect("temp dir");
        let path = Utf8PathBuf::from_path_buf(temp.path().join("payload.json")).expect("utf8");
        std::fs::write(&path, r#"{"name": 123}"#).expect("write");

        let schema = r#"{
            "type": "object",
            "required": ["name"],
            "properties": { "name": { "type": "string" } }
        }"#;

        let outcome = validate_file_if_exists(&path, schema).expect("ok");
        match outcome {
            ValidateOutcome::SchemaErrors(errors) => {
                assert!(!errors.is_empty());
            }
            _ => panic!("expected schema errors"),
        }
    }

    #[test]
    fn validate_file_if_exists_ok_for_valid_payload() {
        let temp = TempDir::new().expect("temp dir");
        let path = Utf8PathBuf::from_path_buf(temp.path().join("payload.json")).expect("utf8");
        std::fs::write(&path, r#"{"name": "ok"}"#).expect("write");

        let schema = r#"{
            "type": "object",
            "required": ["name"],
            "properties": { "name": { "type": "string" } }
        }"#;

        let outcome = validate_file_if_exists(&path, schema).expect("ok");
        assert!(matches!(outcome, ValidateOutcome::Ok));
    }
}
