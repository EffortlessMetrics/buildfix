//! Core plan and apply pipelines, extracted from the CLI.
//!
//! These entry points are I/O-agnostic: all filesystem and git operations
//! are performed through the port traits.

use crate::ports::{GitPort, ReceiptSource, WritePort};
use crate::settings::{ApplySettings, PlanSettings};
use anyhow::Context;
use buildfix_domain::{FsRepoView, PlanContext, Planner, PlannerConfig};
use buildfix_edit::{
    apply_plan, attach_preconditions, preview_patch, ApplyOptions, AttachPreconditionsOptions,
};
use buildfix_receipts::LoadedReceipt;
use buildfix_render::{render_apply_md, render_plan_md};
use buildfix_types::apply::BuildfixApply;
use buildfix_types::plan::BuildfixPlan;
use buildfix_types::receipt::ToolInfo;
use buildfix_types::report::{
    BuildfixReport, InputFailure, ReportArtifacts, ReportCapabilities, ReportCounts, ReportFinding,
    ReportRunInfo, ReportSeverity, ReportStatus, ReportToolInfo, ReportVerdict,
};
use buildfix_types::wire::{PlanV1, ReportV1};
use chrono::Utc;
use sha2::{Digest, Sha256};
use tracing::debug;

/// Error type for pipeline results.  Exit code 2 = policy block, 1 = tool error.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("policy block")]
    PolicyBlock,
    #[error("{0:#}")]
    Internal(#[from] anyhow::Error),
}

/// Outcome of `run_plan`.
pub struct PlanOutcome {
    pub plan: BuildfixPlan,
    pub report: BuildfixReport,
    pub patch: String,
    pub policy_block: bool,
}

/// Run the plan pipeline. Returns the plan, report, and patch.
///
/// The caller is responsible for writing artifacts to disk (via `WritePort`)
/// or the convenience `write_plan_artifacts` helper.
pub fn run_plan(
    settings: &PlanSettings,
    receipts_port: &dyn ReceiptSource,
    git: &dyn GitPort,
    tool: ToolInfo,
) -> Result<PlanOutcome, ToolError> {
    let planner_cfg = PlannerConfig {
        allow: settings.allow.clone(),
        deny: settings.deny.clone(),
        allow_guarded: settings.allow_guarded,
        allow_unsafe: settings.allow_unsafe,
        allow_dirty: settings.allow_dirty,
        max_ops: settings.max_ops,
        max_files: settings.max_files,
        max_patch_bytes: settings.max_patch_bytes,
        params: settings.params.clone(),
    };

    let receipts = receipts_port.load_receipts()?;

    let planner = Planner::new();
    let ctx = PlanContext {
        repo_root: settings.repo_root.clone(),
        artifacts_dir: settings.artifacts_dir.clone(),
        config: planner_cfg.clone(),
    };
    let repo = FsRepoView::new(settings.repo_root.clone());

    let mut plan = planner
        .plan(&ctx, &repo, &receipts, tool.clone())
        .context("generate plan")?;

    // Attach preconditions.
    if settings.require_clean_hashes {
        let attach_opts = AttachPreconditionsOptions {
            include_git_head: settings.git_head_precondition,
        };
        attach_preconditions(&settings.repo_root, &mut plan, &attach_opts)
            .context("attach preconditions")?;
    } else {
        plan.preconditions.files.clear();
    }

    // Populate repo info from git.
    if let Ok(Some(sha)) = git.head_sha(&settings.repo_root) {
        plan.repo.head_sha = Some(sha.clone());
        if settings.git_head_precondition {
            plan.preconditions.head_sha = Some(sha);
        }
    }
    if let Ok(Some(dirty)) = git.is_dirty(&settings.repo_root) {
        plan.repo.dirty = Some(dirty);
        plan.preconditions.dirty = Some(dirty);
    }

    // Preview patch (all unblocked ops, guarded/unsafe included).
    let preview_opts = ApplyOptions {
        dry_run: true,
        allow_guarded: true,
        allow_unsafe: true,
        backup_enabled: false,
        backup_dir: None,
        backup_suffix: settings.backup_suffix.clone(),
        params: settings.params.clone(),
    };
    let mut patch =
        preview_patch(&settings.repo_root, &plan, &preview_opts).context("preview patch")?;

    // Update patch_bytes and enforce max_patch_bytes cap.
    let patch_bytes = patch.len() as u64;
    plan.summary.patch_bytes = Some(patch_bytes);

    if let Some(max_bytes) = planner_cfg.max_patch_bytes
        && patch_bytes > max_bytes {
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

    let report = report_from_plan(&plan, tool, &receipts);
    let policy_block = plan.ops.iter().any(|o| o.blocked);

    Ok(PlanOutcome {
        plan,
        report,
        patch,
        policy_block,
    })
}

/// Write all plan artifacts to the output directory.
pub fn write_plan_artifacts(
    outcome: &PlanOutcome,
    out_dir: &camino::Utf8Path,
    writer: &dyn WritePort,
) -> anyhow::Result<()> {
    writer.create_dir_all(out_dir)?;

    let plan_wire =
        PlanV1::try_from(&outcome.plan).context("convert plan to wire")?;
    let plan_json =
        serde_json::to_string_pretty(&plan_wire).context("serialize plan")?;
    writer.write_file(&out_dir.join("plan.json"), plan_json.as_bytes())?;

    let plan_md = render_plan_md(&outcome.plan);
    writer.write_file(&out_dir.join("plan.md"), plan_md.as_bytes())?;

    writer.write_file(&out_dir.join("patch.diff"), outcome.patch.as_bytes())?;

    let report_wire = ReportV1::from(&outcome.report);
    let report_json =
        serde_json::to_string_pretty(&report_wire).context("serialize report")?;
    writer.write_file(&out_dir.join("report.json"), report_json.as_bytes())?;

    // Write extras.
    let extras_dir = out_dir.join("extras");
    writer.create_dir_all(&extras_dir)?;
    let mut extras_report = outcome.report.clone();
    extras_report.schema = buildfix_types::schema::BUILDFIX_REPORT_V1.to_string();
    let extras_wire = ReportV1::from(&extras_report);
    let extras_json =
        serde_json::to_string_pretty(&extras_wire).context("serialize extras report")?;
    writer.write_file(
        &extras_dir.join("buildfix.report.v1.json"),
        extras_json.as_bytes(),
    )?;

    Ok(())
}

/// Outcome of `run_apply`.
pub struct ApplyOutcome {
    pub apply: BuildfixApply,
    pub report: BuildfixReport,
    pub patch: String,
    pub policy_block: bool,
}

/// Run the apply pipeline. Returns the apply result, report, and patch.
pub fn run_apply(
    settings: &ApplySettings,
    git: &dyn GitPort,
    tool: ToolInfo,
) -> Result<ApplyOutcome, ToolError> {
    let plan_path = settings.out_dir.join("plan.json");
    let plan_str = std::fs::read_to_string(&plan_path)
        .with_context(|| format!("read {}", plan_path))?;

    let plan: BuildfixPlan = match serde_json::from_str::<PlanV1>(&plan_str) {
        Ok(wire) => BuildfixPlan::from(wire),
        Err(err) => {
            debug!("plan.json is not wire format: {}", err);
            serde_json::from_str(&plan_str).context("parse plan.json")?
        }
    };

    let opts = ApplyOptions {
        dry_run: settings.dry_run,
        allow_guarded: settings.allow_guarded,
        allow_unsafe: settings.allow_unsafe,
        backup_enabled: settings.backup_enabled,
        backup_dir: Some(settings.out_dir.join("backups")),
        backup_suffix: settings.backup_suffix.clone(),
        params: settings.params.clone(),
    };

    let mut policy_block_dirty = false;

    // Block apply on dirty working tree unless explicitly allowed.
    if !settings.dry_run && !settings.allow_dirty
        && let Ok(Some(true)) = git.is_dirty(&settings.repo_root) {
            policy_block_dirty = true;
        }

    let (mut apply, patch) = if policy_block_dirty {
        let mut apply = empty_apply_from_plan(&plan, &settings.repo_root, tool.clone(), &plan_path);
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
        apply_plan(&settings.repo_root, &plan, tool.clone(), &opts).context("apply plan")?
    };

    // Populate plan_ref and repo info.
    apply.plan_ref = buildfix_types::apply::PlanRef {
        path: plan_path.to_string(),
        sha256: Some(sha256_hex(plan_str.as_bytes())),
    };
    apply.repo = buildfix_types::apply::ApplyRepoInfo {
        root: settings.repo_root.to_string(),
        head_sha_before: git.head_sha(&settings.repo_root).ok().flatten(),
        head_sha_after: git.head_sha(&settings.repo_root).ok().flatten(),
        dirty_before: git.is_dirty(&settings.repo_root).ok().flatten(),
        dirty_after: git.is_dirty(&settings.repo_root).ok().flatten(),
    };

    let report = report_from_apply(&apply, tool);
    let policy_block =
        buildfix_edit::check_policy_block(&apply, settings.dry_run).is_some();

    Ok(ApplyOutcome {
        apply,
        report,
        patch,
        policy_block,
    })
}

/// Write all apply artifacts to the output directory.
pub fn write_apply_artifacts(
    outcome: &ApplyOutcome,
    out_dir: &camino::Utf8Path,
    writer: &dyn WritePort,
) -> anyhow::Result<()> {
    writer.create_dir_all(out_dir)?;

    let apply_wire =
        buildfix_types::wire::ApplyV1::try_from(&outcome.apply).context("convert apply to wire")?;
    let apply_json =
        serde_json::to_string_pretty(&apply_wire).context("serialize apply")?;
    writer.write_file(&out_dir.join("apply.json"), apply_json.as_bytes())?;

    let apply_md = render_apply_md(&outcome.apply);
    writer.write_file(&out_dir.join("apply.md"), apply_md.as_bytes())?;

    writer.write_file(&out_dir.join("patch.diff"), outcome.patch.as_bytes())?;

    let report_wire = ReportV1::from(&outcome.report);
    let report_json =
        serde_json::to_string_pretty(&report_wire).context("serialize report")?;
    writer.write_file(&out_dir.join("report.json"), report_json.as_bytes())?;

    // Write extras.
    let extras_dir = out_dir.join("extras");
    writer.create_dir_all(&extras_dir)?;
    let mut extras_report = outcome.report.clone();
    extras_report.schema = buildfix_types::schema::BUILDFIX_REPORT_V1.to_string();
    let extras_wire = ReportV1::from(&extras_report);
    let extras_json =
        serde_json::to_string_pretty(&extras_wire).context("serialize extras report")?;
    writer.write_file(
        &extras_dir.join("buildfix.report.v1.json"),
        extras_json.as_bytes(),
    )?;

    Ok(())
}

// ── report helpers (extracted from CLI) ──────────────────────────────────

pub(crate) fn report_from_plan(
    plan: &BuildfixPlan,
    tool: ToolInfo,
    receipts: &[LoadedReceipt],
) -> BuildfixReport {
    let capabilities = build_capabilities(receipts);
    let has_failed_inputs = !capabilities.inputs_failed.is_empty();

    let status = if plan.ops.is_empty() && !has_failed_inputs {
        ReportStatus::Pass
    } else {
        ReportStatus::Warn
    };

    let mut reasons = Vec::new();
    if has_failed_inputs {
        reasons.push("partial_inputs".to_string());
    }

    let mut findings: Vec<ReportFinding> = Vec::new();
    for failure in &capabilities.inputs_failed {
        findings.push(ReportFinding {
            severity: ReportSeverity::Warn,
            check_id: Some("inputs".to_string()),
            code: "receipt_load_failed".to_string(),
            message: format!(
                "Receipt failed to load: {} ({})",
                failure.path, failure.reason
            ),
            location: None,
            fingerprint: Some(format!("inputs/receipt_load_failed/{}", failure.path)),
            data: None,
        });
    }

    let warn_count = plan.ops.len() as u64 + capabilities.inputs_failed.len() as u64;

    BuildfixReport {
        schema: buildfix_types::schema::SENSOR_REPORT_V1.to_string(),
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
                warn: warn_count,
                error: 0,
            },
            reasons,
        },
        findings,
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

fn build_capabilities(receipts: &[LoadedReceipt]) -> ReportCapabilities {
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

pub(crate) fn report_from_apply(apply: &BuildfixApply, tool: ToolInfo) -> BuildfixReport {
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
        schema: buildfix_types::schema::SENSOR_REPORT_V1.to_string(),
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
        capabilities: None,
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

fn empty_apply_from_plan(
    _plan: &BuildfixPlan,
    repo_root: &camino::Utf8Path,
    tool: ToolInfo,
    plan_path: &camino::Utf8Path,
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

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
