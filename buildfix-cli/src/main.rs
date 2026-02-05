mod config;
mod explain;

use anyhow::Context;
use buildfix_domain::{FsRepoView, PlanContext, Planner, PlannerConfig};
use buildfix_edit::{
    apply_plan, attach_preconditions, is_working_tree_dirty, preview_patch, ApplyOptions,
    AttachPreconditionsOptions,
};
use buildfix_render::{render_apply_md, render_plan_md};
use buildfix_types::apply::BuildfixApply;
use buildfix_types::plan::BuildfixPlan;
use buildfix_types::receipt::ToolInfo;
use buildfix_types::report::{
    BuildfixReport, InputFailure, ReportArtifacts, ReportCapabilities, ReportCounts, ReportRunInfo,
    ReportStatus, ReportToolInfo, ReportVerdict,
};
use buildfix_types::wire::{ApplyV1, PlanV1, ReportV1};
use camino::{Utf8Path, Utf8PathBuf};
use chrono::Utc;
use clap::{Parser, Subcommand};
use config::{parse_cli_params, ConfigMerger};
use fs_err as fs;
use jsonschema::JSONSchema;
use sha2::{Digest, Sha256};
use std::process::ExitCode;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

const PLAN_SCHEMA: &str = include_str!("../schemas/buildfix.plan.v1.json");
const APPLY_SCHEMA: &str = include_str!("../schemas/buildfix.apply.v1.json");
const REPORT_SCHEMA: &str = include_str!("../schemas/buildfix.report.v1.json");

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

    fs::create_dir_all(&out_dir).with_context(|| format!("create {}", out_dir))?;

    let cli_params = parse_cli_params(&args.param)?;

    // Load config file and merge with CLI arguments
    let file_config = config::load_or_default(&repo_root).context("load buildfix.toml config")?;
    let merged = ConfigMerger::new(file_config).merge_plan_args(
        &args.allow,
        &args.deny,
        args.no_clean_hashes,
        &cli_params,
    );

    let planner_cfg = PlannerConfig {
        allow: merged.allow.clone(),
        deny: merged.deny.clone(),
        allow_guarded: merged.allow_guarded,
        allow_unsafe: merged.allow_unsafe,
        allow_dirty: merged.allow_dirty,
        max_ops: args.max_ops.or(merged.max_ops),
        max_files: args.max_files.or(merged.max_files),
        max_patch_bytes: args.max_patch_bytes.or(merged.max_patch_bytes),
        params: merged.params.clone(),
    };

    debug!(
        "merged config: allow={:?}, deny={:?}, require_clean_hashes={}, params={:?}",
        merged.allow, merged.deny, merged.require_clean_hashes, merged.params
    );

    let receipts = buildfix_receipts::load_receipts(&artifacts_dir)
        .with_context(|| format!("load receipts from {}", artifacts_dir))?;

    let planner = Planner::new();
    let ctx = PlanContext {
        repo_root: repo_root.clone(),
        artifacts_dir: artifacts_dir.clone(),
        config: planner_cfg.clone(),
    };
    let repo = FsRepoView::new(repo_root.clone());
    let tool = tool_info();

    let mut plan = planner
        .plan(&ctx, &repo, &receipts, tool.clone())
        .context("generate plan")?;

    if merged.require_clean_hashes {
        let attach_opts = AttachPreconditionsOptions {
            include_git_head: args.git_head_precondition,
        };
        attach_preconditions(&repo_root, &mut plan, &attach_opts)
            .context("attach preconditions")?;
    } else {
        plan.preconditions.files.clear();
    }

    if let Ok(sha) = buildfix_edit::get_head_sha(&repo_root) {
        plan.repo.head_sha = Some(sha.clone());
        if args.git_head_precondition {
            plan.preconditions.head_sha = Some(sha);
        }
    }
    if let Ok(dirty) = is_working_tree_dirty(&repo_root) {
        plan.repo.dirty = Some(dirty);
        plan.preconditions.dirty = Some(dirty);
    }

    // Preview patch with all unblocked ops (guarded/unsafe included).
    let preview_opts = ApplyOptions {
        dry_run: true,
        allow_guarded: true,
        allow_unsafe: true,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: merged.backups.suffix.clone(),
        params: merged.params.clone(),
    };
    let mut patch = preview_patch(&repo_root, &plan, &preview_opts).context("preview patch")?;

    // Update patch_bytes summary and enforce max_patch_bytes cap.
    let patch_bytes = patch.len() as u64;
    plan.summary.patch_bytes = Some(patch_bytes);

    if let Some(max_bytes) = planner_cfg.max_patch_bytes {
        if patch_bytes > max_bytes {
            for op in plan.ops.iter_mut() {
                op.blocked = true;
                op.blocked_reason = Some(format!(
                    "caps exceeded: max_patch_bytes {} > {} allowed",
                    patch_bytes, max_bytes
                ));
            }
            plan.summary.ops_blocked = plan.ops.len() as u64;
            plan.summary.patch_bytes = Some(0);
            patch.clear();
        }
    }

    let plan_wire = PlanV1::try_from(&plan).context("convert plan to wire")?;
    write_json(&out_dir.join("plan.json"), &plan_wire)?;
    fs::write(out_dir.join("plan.md"), render_plan_md(&plan))?;
    fs::write(out_dir.join("patch.diff"), &patch)?;

    let report = report_from_plan(&plan, tool, &receipts);
    let report_wire = ReportV1::from(&report);
    write_json(&out_dir.join("report.json"), &report_wire)?;

    info!("wrote plan to {}", out_dir);

    let policy_block = plan.ops.iter().any(|o| o.blocked);
    Ok(if policy_block {
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

    // Load config file and merge with CLI arguments
    let file_config = config::load_or_default(&repo_root).context("load buildfix.toml config")?;
    let merged = ConfigMerger::new(file_config).merge_apply_args(
        args.allow_guarded,
        args.allow_unsafe,
        &cli_params,
    );

    // Determine allow_dirty: CLI flag OR config file setting
    let allow_dirty = args.allow_dirty || merged.allow_dirty;

    debug!(
        "merged config: allow_guarded={}, allow_unsafe={}, allow_dirty={}",
        merged.allow_guarded, merged.allow_unsafe, allow_dirty
    );

    let plan_path = out_dir.join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).with_context(|| format!("read {}", plan_path))?;
    let plan: BuildfixPlan = match serde_json::from_str::<PlanV1>(&plan_str) {
        Ok(wire) => BuildfixPlan::from(wire),
        Err(err) => {
            debug!("plan.json is not wire format: {}", err);
            serde_json::from_str(&plan_str).context("parse plan.json")?
        }
    };

    let opts = ApplyOptions {
        dry_run: !args.apply,
        allow_guarded: merged.allow_guarded,
        allow_unsafe: merged.allow_unsafe,
        backup_enabled: merged.backups.enabled,
        backup_dir: Some(out_dir.join("backups")),
        backup_suffix: merged.backups.suffix.clone(),
        params: merged.params.clone(),
    };

    let mut policy_block = false;

    // Block apply on dirty working tree unless explicitly allowed
    if args.apply && !allow_dirty {
        if let Ok(true) = is_working_tree_dirty(&repo_root) {
            policy_block = true;
        }
    }

    let tool = tool_info();

    let (mut apply, patch) = if policy_block {
        let mut apply = empty_apply_from_plan(&plan, &repo_root, tool.clone(), &plan_path);
        apply.preconditions.verified = false;
        apply
            .preconditions
            .mismatches
            .push(buildfix_types::apply::PreconditionMismatch {
                path: "<working_tree>".to_string(),
                expected: "clean".to_string(),
                actual: "dirty".to_string(),
            });
        for op in &plan.ops {
            apply.results.push(buildfix_types::apply::ApplyResult {
                op_id: op.id.clone(),
                status: buildfix_types::apply::ApplyStatus::Blocked,
                message: Some("dirty working tree".to_string()),
                blocked_reason: Some("dirty working tree".to_string()),
                files: vec![],
            });
        }
        apply.summary.blocked = plan.ops.len() as u64;
        (apply, String::new())
    } else {
        apply_plan(&repo_root, &plan, tool.clone(), &opts).context("apply plan")?
    };

    // Populate plan_ref sha256 and repo info
    apply.plan_ref = buildfix_types::apply::PlanRef {
        path: plan_path.to_string(),
        sha256: Some(sha256_hex(plan_str.as_bytes())),
    };

    apply.repo = buildfix_types::apply::ApplyRepoInfo {
        root: repo_root.to_string(),
        head_sha_before: buildfix_edit::get_head_sha(&repo_root).ok(),
        head_sha_after: buildfix_edit::get_head_sha(&repo_root).ok(),
        dirty_before: is_working_tree_dirty(&repo_root).ok(),
        dirty_after: is_working_tree_dirty(&repo_root).ok(),
    };

    let apply_wire = ApplyV1::try_from(&apply).context("convert apply to wire")?;
    write_json(&out_dir.join("apply.json"), &apply_wire)?;
    fs::write(out_dir.join("apply.md"), render_apply_md(&apply))?;
    fs::write(out_dir.join("patch.diff"), &patch)?;

    let report = report_from_apply(&apply, tool);
    let report_wire = ReportV1::from(&report);
    write_json(&out_dir.join("report.json"), &report_wire)?;

    info!("wrote apply artifacts to {}", out_dir);

    let policy_block = buildfix_edit::check_policy_block(&apply, !args.apply).is_some();
    Ok(if policy_block {
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
    let compiled = JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema_json)
        .map_err(|e| anyhow::anyhow!("compile schema: {}", e))?;
    if let Err(errors) = compiled.validate(&json) {
        let msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
        return Ok(ValidateOutcome::SchemaErrors(msgs));
    }
    Ok(ValidateOutcome::Ok)
}

fn write_json<T: serde::Serialize>(path: &Utf8Path, v: &T) -> anyhow::Result<()> {
    let s = serde_json::to_string_pretty(v).context("serialize json")?;
    fs::write(path, s).with_context(|| format!("write {}", path))?;
    Ok(())
}

fn tool_info() -> ToolInfo {
    ToolInfo {
        name: "buildfix".to_string(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        repo: None,
        commit: None,
    }
}

fn report_from_plan(
    plan: &BuildfixPlan,
    tool: ToolInfo,
    receipts: &[buildfix_receipts::LoadedReceipt],
) -> BuildfixReport {
    let status = if plan.ops.is_empty() {
        ReportStatus::Pass
    } else {
        ReportStatus::Warn
    };

    let capabilities = build_capabilities(receipts);

    BuildfixReport {
        schema: buildfix_types::schema::BUILDFIX_REPORT_V1.to_string(),
        tool: ReportToolInfo {
            name: tool.name,
            version: tool.version.unwrap_or_else(|| "unknown".to_string()),
            commit: tool.commit,
        },
        run: ReportRunInfo {
            started_at: Utc::now().to_rfc3339(),
            ended_at: Some(Utc::now().to_rfc3339()),
            duration_ms: Some(0),
        },
        verdict: ReportVerdict {
            status,
            counts: ReportCounts {
                info: 0,
                warn: plan.ops.len() as u64,
                error: 0,
            },
            reasons: vec![],
        },
        findings: vec![],
        capabilities: Some(capabilities),
        artifacts: Some(ReportArtifacts {
            plan: Some("plan.json".to_string()),
            apply: None,
            patch: Some("patch.diff".to_string()),
        }),
        data: Some(serde_json::json!({
            "ops_total": plan.summary.ops_total,
            "ops_blocked": plan.summary.ops_blocked,
            "files_touched": plan.summary.files_touched,
            "patch_bytes": plan.summary.patch_bytes,
        })),
    }
}

fn build_capabilities(receipts: &[buildfix_receipts::LoadedReceipt]) -> ReportCapabilities {
    let mut inputs_available = Vec::new();
    let mut inputs_failed = Vec::new();

    for r in receipts {
        match &r.receipt {
            Ok(_) => {
                inputs_available.push(r.path.to_string());
            }
            Err(e) => {
                inputs_failed.push(InputFailure {
                    path: r.path.to_string(),
                    reason: e.to_string(),
                });
            }
        }
    }

    ReportCapabilities {
        inputs_available,
        inputs_failed,
    }
}

fn report_from_apply(apply: &BuildfixApply, tool: ToolInfo) -> BuildfixReport {
    let status = if apply.summary.failed > 0 {
        ReportStatus::Fail
    } else if apply.summary.blocked > 0 {
        ReportStatus::Warn
    } else if apply.summary.applied > 0 {
        ReportStatus::Pass
    } else {
        ReportStatus::Warn
    };

    BuildfixReport {
        schema: buildfix_types::schema::BUILDFIX_REPORT_V1.to_string(),
        tool: ReportToolInfo {
            name: tool.name,
            version: tool.version.unwrap_or_else(|| "unknown".to_string()),
            commit: tool.commit,
        },
        run: ReportRunInfo {
            started_at: Utc::now().to_rfc3339(),
            ended_at: Some(Utc::now().to_rfc3339()),
            duration_ms: Some(0),
        },
        verdict: ReportVerdict {
            status,
            counts: ReportCounts {
                info: apply.summary.applied,
                warn: apply.summary.blocked,
                error: apply.summary.failed,
            },
            reasons: vec![],
        },
        findings: vec![],
        capabilities: None, // Apply doesn't have receipt context
        artifacts: Some(ReportArtifacts {
            plan: Some("plan.json".to_string()),
            apply: Some("apply.json".to_string()),
            patch: Some("patch.diff".to_string()),
        }),
        data: Some(serde_json::json!({
            "attempted": apply.summary.attempted,
            "applied": apply.summary.applied,
            "blocked": apply.summary.blocked,
            "failed": apply.summary.failed,
            "files_modified": apply.summary.files_modified,
        })),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn empty_apply_from_plan(
    _plan: &BuildfixPlan,
    repo_root: &Utf8Path,
    tool: ToolInfo,
    plan_path: &Utf8Path,
) -> BuildfixApply {
    let repo_info = buildfix_types::apply::ApplyRepoInfo {
        root: repo_root.to_string(),
        head_sha_before: None,
        head_sha_after: None,
        dirty_before: None,
        dirty_after: None,
    };
    let plan_ref = buildfix_types::apply::PlanRef {
        path: plan_path.to_string(),
        sha256: None,
    };
    BuildfixApply::new(tool, repo_info, plan_ref)
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
    use explain::{format_safety_class, policy_keys, FIX_REGISTRY};

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
