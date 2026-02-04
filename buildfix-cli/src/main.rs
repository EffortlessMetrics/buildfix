mod config;
mod explain;

use anyhow::Context;
use buildfix_domain::{FsRepoView, PlanContext, Planner, PlannerConfig};
use buildfix_edit::{
    apply_plan, attach_preconditions, is_working_tree_dirty, preview_patch, ApplyOptions,
    AttachPreconditionsOptions,
};
use buildfix_render::{render_apply_md, render_plan_md};
use buildfix_types::plan::PolicyCaps;
use buildfix_types::receipt::{RunInfo, ToolInfo, Verdict, VerdictStatus};
use buildfix_types::report::BuildfixReport;
use camino::{Utf8Path, Utf8PathBuf};
use chrono::Utc;
use clap::{Parser, Subcommand};
use config::ConfigMerger;
use fs_err as fs;
use std::process::ExitCode;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;

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

    /// Allowlist patterns for fix ids (apply-time).
    #[arg(long)]
    allow: Vec<String>,

    /// Denylist patterns for fix ids (apply-time).
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

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    if let Err(e) = real_main() {
        error!("{:?}", e);
        return ExitCode::from(1);
    }
    ExitCode::from(0)
}

fn real_main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Command::Plan(args) => cmd_plan(args),
        Command::Apply(args) => cmd_apply(args),
        Command::Explain(args) => cmd_explain(args),
        Command::ListFixes(args) => cmd_list_fixes(args),
    }
}

fn cmd_plan(args: PlanArgs) -> anyhow::Result<()> {
    let repo_root = args.repo_root;
    let artifacts_dir = args
        .artifacts_dir
        .unwrap_or_else(|| repo_root.join("artifacts"));
    let out_dir = args
        .out_dir
        .unwrap_or_else(|| artifacts_dir.join("buildfix"));

    fs::create_dir_all(&out_dir).with_context(|| format!("create {}", out_dir))?;

    // Load config file and merge with CLI arguments
    let file_config = config::load_or_default(&repo_root).context("load buildfix.toml config")?;
    let merged = ConfigMerger::new(file_config).merge_plan_args(
        &args.allow,
        &args.deny,
        args.no_clean_hashes,
    );

    // Build caps from CLI args (override config file values if provided)
    let caps = PolicyCaps {
        max_ops: args.max_ops.or(merged.max_ops),
        max_files: args.max_files.or(merged.max_files),
        max_patch_bytes: args.max_patch_bytes.or(merged.max_patch_bytes),
    };

    debug!(
        "merged config: allow={:?}, deny={:?}, require_clean_hashes={}, caps={:?}",
        merged.allow, merged.deny, merged.require_clean_hashes, caps
    );

    let receipts = buildfix_receipts::load_receipts(&artifacts_dir)
        .with_context(|| format!("load receipts from {}", artifacts_dir))?;

    let planner = Planner::new();
    let ctx = PlanContext {
        repo_root: repo_root.clone(),
        artifacts_dir: artifacts_dir.clone(),
        config: PlannerConfig {
            allow: merged.allow.clone(),
            deny: merged.deny.clone(),
            require_clean_hashes: merged.require_clean_hashes,
            caps: caps.clone(),
        },
    };
    let repo = FsRepoView::new(repo_root.clone());
    let tool = tool_info();

    let mut plan = planner
        .plan(&ctx, &repo, &receipts, tool.clone())
        .context("generate plan")?;

    let attach_opts = AttachPreconditionsOptions {
        include_git_head: args.git_head_precondition,
    };
    attach_preconditions(&repo_root, &mut plan, &attach_opts).context("attach preconditions")?;

    // Preview patch with guarded fixes included; unsafe remains gated unless explicitly allowed.
    let preview_opts = ApplyOptions {
        dry_run: true,
        allow_guarded: true,
        allow_unsafe: false,
        backup_dir: None,
    };
    let patch = preview_patch(&repo_root, &plan, &preview_opts).context("preview patch")?;

    // Enforce max_patch_bytes cap after patch generation
    if let Some(max_bytes) = caps.max_patch_bytes {
        let patch_bytes = patch.len() as u64;
        if patch_bytes > max_bytes {
            anyhow::bail!(
                "patch exceeds max_patch_bytes cap: {} bytes > {} allowed",
                patch_bytes,
                max_bytes
            );
        }
    }

    write_json(&out_dir.join("plan.json"), &plan)?;
    fs::write(out_dir.join("plan.md"), render_plan_md(&plan))?;
    fs::write(out_dir.join("patch.diff"), &patch)?;

    let report = report_from_plan(&plan, tool);
    write_json(&out_dir.join("report.json"), &report)?;

    info!("wrote plan to {}", out_dir);
    Ok(())
}

fn cmd_apply(args: ApplyArgs) -> anyhow::Result<()> {
    let repo_root = args.repo_root;
    let out_dir = args
        .out_dir
        .unwrap_or_else(|| repo_root.join("artifacts").join("buildfix"));

    // Load config file and merge with CLI arguments
    let file_config = config::load_or_default(&repo_root).context("load buildfix.toml config")?;
    let merged =
        ConfigMerger::new(file_config).merge_apply_args(args.allow_guarded, args.allow_unsafe);

    // Determine allow_dirty: CLI flag OR config file setting
    let allow_dirty = args.allow_dirty || merged.allow_dirty;

    debug!(
        "merged config: allow_guarded={}, allow_unsafe={}, allow_dirty={}",
        merged.allow_guarded, merged.allow_unsafe, allow_dirty
    );

    // Block apply on dirty working tree unless explicitly allowed
    if args.apply && !allow_dirty {
        match is_working_tree_dirty(&repo_root) {
            Ok(true) => {
                anyhow::bail!(
                    "git working tree has uncommitted changes; \
                     commit or stash changes first, or use --allow-dirty to override"
                );
            }
            Ok(false) => {
                debug!("working tree is clean");
            }
            Err(e) => {
                // If git isn't available, log warning but don't block
                debug!("could not check git status: {}", e);
            }
        }
    }

    let plan_path = out_dir.join("plan.json");
    let plan_str = fs::read_to_string(&plan_path).with_context(|| format!("read {}", plan_path))?;
    let plan: buildfix_types::plan::BuildfixPlan =
        serde_json::from_str(&plan_str).context("parse plan.json")?;

    let opts = ApplyOptions {
        dry_run: !args.apply,
        allow_guarded: merged.allow_guarded,
        allow_unsafe: merged.allow_unsafe,
        backup_dir: Some(out_dir.join("backups")),
    };

    let tool = tool_info();
    let (apply, patch) =
        apply_plan(&repo_root, &plan, tool.clone(), &opts).context("apply plan")?;

    write_json(&out_dir.join("apply.json"), &apply)?;
    fs::write(out_dir.join("apply.md"), render_apply_md(&apply))?;
    fs::write(out_dir.join("patch.diff"), &patch)?;

    let report = report_from_apply(&apply, tool);
    write_json(&out_dir.join("report.json"), &report)?;

    info!("wrote apply artifacts to {}", out_dir);
    Ok(())
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

fn report_from_plan(plan: &buildfix_types::plan::BuildfixPlan, tool: ToolInfo) -> BuildfixReport {
    let status = if plan.fixes.is_empty() {
        VerdictStatus::Pass
    } else {
        VerdictStatus::Warn
    };

    BuildfixReport {
        schema: buildfix_types::schema::BUILDFIX_REPORT_V1.to_string(),
        tool,
        run: RunInfo {
            started_at: Some(Utc::now()),
            ended_at: Some(Utc::now()),
            git_head_sha: None,
        },
        verdict: Verdict {
            status,
            counts: buildfix_types::receipt::Counts {
                findings: plan.fixes.len() as u64,
                errors: 0,
                warnings: if plan.fixes.is_empty() { 0 } else { 1 },
            },
            reasons: vec![],
        },
        findings: vec![],
        data: Some(serde_json::json!({
            "plan_id": plan.plan_id,
            "fixes_total": plan.summary.fixes_total,
            "safe": plan.summary.safe,
            "guarded": plan.summary.guarded,
            "unsafe": plan.summary.unsafe_,
        })),
    }
}

fn report_from_apply(
    apply: &buildfix_types::apply::BuildfixApply,
    tool: ToolInfo,
) -> BuildfixReport {
    let status = if apply.summary.failed > 0 {
        VerdictStatus::Fail
    } else if apply.summary.applied > 0 {
        VerdictStatus::Pass
    } else {
        VerdictStatus::Warn
    };

    BuildfixReport {
        schema: buildfix_types::schema::BUILDFIX_REPORT_V1.to_string(),
        tool,
        run: RunInfo {
            started_at: Some(Utc::now()),
            ended_at: Some(Utc::now()),
            git_head_sha: None,
        },
        verdict: Verdict {
            status,
            counts: buildfix_types::receipt::Counts {
                findings: apply.results.len() as u64,
                errors: apply.summary.failed,
                warnings: apply.summary.skipped,
            },
            reasons: vec![],
        },
        findings: vec![],
        data: Some(serde_json::json!({
            "plan_id": apply.plan_id,
            "attempted": apply.summary.attempted,
            "applied": apply.summary.applied,
            "skipped": apply.summary.skipped,
            "failed": apply.summary.failed,
        })),
    }
}

fn cmd_explain(args: ExplainArgs) -> anyhow::Result<()> {
    use explain::{format_safety_class, list_fix_keys, lookup_fix, safety_class_meaning};

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
    use explain::{format_safety_class, FIX_REGISTRY};

    match args.format {
        OutputFormat::Text => {
            println!("Available fixes:\n");
            println!("  {:<24} {:<10} TITLE", "KEY", "SAFETY");
            println!("  {:<24} {:<10} -----", "---", "------");
            for fix in FIX_REGISTRY {
                println!(
                    "  {:<24} {:<10} {}",
                    fix.key,
                    format_safety_class(fix.safety),
                    fix.title
                );
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
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&fixes)?);
        }
    }
    Ok(())
}
