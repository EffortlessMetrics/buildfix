//! Core plan and apply pipelines, extracted from the CLI.
//!
//! These entry points are I/O-agnostic: all filesystem and git operations
//! are performed through the port traits.

use crate::ports::{GitPort, ReceiptSource, WritePort};
use crate::settings::{ApplySettings, PlanSettings};
use anyhow::Context;
use buildfix_domain::{FsRepoView, PlanContext, Planner, PlannerConfig};
use buildfix_edit::{
    ApplyOptions, AttachPreconditionsOptions, apply_plan, attach_preconditions, preview_patch,
};
use buildfix_receipts::LoadedReceipt;
use buildfix_render::{render_apply_md, render_comment_md, render_plan_md};
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
        && patch_bytes > max_bytes
    {
        for op in plan.ops.iter_mut() {
            op.blocked = true;
            op.blocked_reason = Some(format!(
                "caps exceeded: max_patch_bytes {} > {} allowed",
                patch_bytes, max_bytes
            ));
            op.blocked_reason_token =
                Some(buildfix_types::plan::blocked_tokens::MAX_PATCH_BYTES.to_string());
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

    let plan_wire = PlanV1::try_from(&outcome.plan).context("convert plan to wire")?;
    let plan_json = serde_json::to_string_pretty(&plan_wire).context("serialize plan")?;
    writer.write_file(&out_dir.join("plan.json"), plan_json.as_bytes())?;

    let plan_md = render_plan_md(&outcome.plan);
    writer.write_file(&out_dir.join("plan.md"), plan_md.as_bytes())?;

    let comment_md = render_comment_md(&outcome.plan);
    writer.write_file(&out_dir.join("comment.md"), comment_md.as_bytes())?;

    writer.write_file(&out_dir.join("patch.diff"), outcome.patch.as_bytes())?;

    let report_wire = ReportV1::from(&outcome.report);
    let report_json = serde_json::to_string_pretty(&report_wire).context("serialize report")?;
    writer.write_file(&out_dir.join("report.json"), report_json.as_bytes())?;

    // Write extras.
    let extras_dir = out_dir.join("extras");
    writer.create_dir_all(&extras_dir)?;
    let mut extras_report = outcome.report.clone();
    extras_report.schema = buildfix_types::schema::BUILDFIX_REPORT_V1.to_string();
    if let Some(ref mut artifacts) = extras_report.artifacts {
        artifacts.comment = Some("comment.md".to_string());
    }
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
    let plan_str =
        std::fs::read_to_string(&plan_path).with_context(|| format!("read {}", plan_path))?;

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
    if !settings.dry_run
        && !settings.allow_dirty
        && let Ok(Some(true)) = git.is_dirty(&settings.repo_root)
    {
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
                blocked_reason_token: Some(
                    buildfix_types::plan::blocked_tokens::DIRTY_WORKING_TREE.to_string(),
                ),
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
    let policy_block = buildfix_edit::check_policy_block(&apply, settings.dry_run).is_some();

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
    let apply_json = serde_json::to_string_pretty(&apply_wire).context("serialize apply")?;
    writer.write_file(&out_dir.join("apply.json"), apply_json.as_bytes())?;

    let apply_md = render_apply_md(&outcome.apply);
    writer.write_file(&out_dir.join("apply.md"), apply_md.as_bytes())?;

    writer.write_file(&out_dir.join("patch.diff"), outcome.patch.as_bytes())?;

    let report_wire = ReportV1::from(&outcome.report);
    let report_json = serde_json::to_string_pretty(&report_wire).context("serialize report")?;
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
            comment: None,
        }),
        data: Some({
            let ops_applicable = plan
                .summary
                .ops_total
                .saturating_sub(plan.summary.ops_blocked);
            let fix_available = ops_applicable > 0;
            let mut plan_data = serde_json::json!({
                "ops_total": plan.summary.ops_total,
                "ops_blocked": plan.summary.ops_blocked,
                "ops_applicable": ops_applicable,
                "fix_available": fix_available,
                "files_touched": plan.summary.files_touched,
                "patch_bytes": plan.summary.patch_bytes,
                "plan_available": !plan.ops.is_empty(),
            });
            if let Some(sc) = &plan.summary.safety_counts {
                plan_data["safety_counts"] = serde_json::json!({
                    "safe": sc.safe,
                    "guarded": sc.guarded,
                    "unsafe": sc.unsafe_count,
                });
            }
            let tokens: std::collections::BTreeSet<&str> = plan
                .ops
                .iter()
                .filter_map(|o| o.blocked_reason_token.as_deref())
                .collect();
            let top: Vec<&str> = tokens.into_iter().take(5).collect();
            if !top.is_empty() {
                plan_data["blocked_reason_tokens_top"] = serde_json::json!(top);
            }
            serde_json::json!({
                "buildfix": {
                    "plan": plan_data
                }
            })
        }),
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
            comment: None,
        }),
        data: Some(serde_json::json!({
            "buildfix": {
                "apply": {
                    "attempted": apply.summary.attempted,
                    "applied": apply.summary.applied,
                    "blocked": apply.summary.blocked,
                    "failed": apply.summary.failed,
                    "files_modified": apply.summary.files_modified,
                    "apply_performed": apply.summary.applied > 0,
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use buildfix_receipts::{LoadedReceipt, ReceiptLoadError};
    use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
    use buildfix_types::plan::{
        PlanOp, PlanPolicy, PlanSummary, Rationale, RepoInfo, SafetyCounts,
    };
    use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, RunInfo, ToolInfo, Verdict};
    use buildfix_types::wire::PlanV1;
    use camino::{Utf8Path, Utf8PathBuf};
    use crate::settings::RunMode;
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tempfile::TempDir;

    #[derive(Default)]
    struct StubGitPort {
        head: Option<String>,
        dirty: Option<bool>,
    }

    impl GitPort for StubGitPort {
        fn head_sha(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<String>> {
            Ok(self.head.clone())
        }

        fn is_dirty(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<bool>> {
            Ok(self.dirty)
        }
    }

    #[derive(Default)]
    struct MemWritePort {
        files: Mutex<HashMap<String, Vec<u8>>>,
        dirs: Mutex<Vec<String>>,
    }

    impl WritePort for MemWritePort {
        fn write_file(&self, path: &Utf8Path, contents: &[u8]) -> anyhow::Result<()> {
            let key = path.as_str().replace('\\', "/");
            self.files
                .lock()
                .expect("lock files")
                .insert(key, contents.to_vec());
            Ok(())
        }

        fn create_dir_all(&self, path: &Utf8Path) -> anyhow::Result<()> {
            let key = path.as_str().replace('\\', "/");
            self.dirs.lock().expect("lock dirs").push(key);
            Ok(())
        }
    }

    fn tool() -> ToolInfo {
        ToolInfo {
            name: "buildfix".into(),
            version: Some("0.0.0-test".into()),
            repo: None,
            commit: None,
        }
    }

    fn make_plan(ops: Vec<PlanOp>, safety_counts: Option<SafetyCounts>) -> BuildfixPlan {
        let mut plan = BuildfixPlan::new(
            tool(),
            RepoInfo {
                root: ".".into(),
                head_sha: None,
                dirty: None,
            },
            PlanPolicy::default(),
        );
        let ops_total = ops.len() as u64;
        let ops_blocked = ops.iter().filter(|o| o.blocked).count() as u64;
        plan.summary = PlanSummary {
            ops_total,
            ops_blocked,
            files_touched: 1,
            patch_bytes: Some(0),
            safety_counts,
        };
        plan.ops = ops;
        plan
    }

    fn make_op(safety: SafetyClass, blocked: bool, blocked_reason: Option<&str>) -> PlanOp {
        make_op_with_token(safety, blocked, blocked_reason, None)
    }

    fn make_op_with_token(
        safety: SafetyClass,
        blocked: bool,
        blocked_reason: Option<&str>,
        blocked_reason_token: Option<&str>,
    ) -> PlanOp {
        PlanOp {
            id: "test-op".into(),
            safety,
            blocked,
            blocked_reason: blocked_reason.map(|s| s.to_string()),
            blocked_reason_token: blocked_reason_token.map(|s| s.to_string()),
            target: OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: OpKind::TomlSet {
                toml_path: vec!["workspace".into(), "resolver".into()],
                value: serde_json::json!("2"),
            },
            rationale: Rationale {
                fix_key: "test".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        }
    }

    fn create_temp_repo(manifest_contents: &str) -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().expect("temp dir");
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
        std::fs::write(root.join("Cargo.toml"), manifest_contents).expect("write manifest");
        (temp, root)
    }

    fn resolver_receipt() -> LoadedReceipt {
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: ToolInfo {
                name: "builddiag".to_string(),
                version: Some("1.0.0".to_string()),
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("workspace.resolver_v2".to_string()),
                code: Some("RESOLVER".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from("Cargo.toml"),
                    line: Some(1),
                    column: None,
                }),
                fingerprint: None,
                data: None,
            }],
            capabilities: None,
            data: None,
        };

        LoadedReceipt {
            path: Utf8PathBuf::from("artifacts/builddiag/report.json"),
            sensor_id: "builddiag".to_string(),
            receipt: Ok(receipt),
        }
    }

    fn build_plan_settings(root: &Utf8Path) -> PlanSettings {
        PlanSettings {
            repo_root: root.to_path_buf(),
            artifacts_dir: root.join("artifacts"),
            out_dir: root.join("artifacts/buildfix"),
            allow: Vec::new(),
            deny: Vec::new(),
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: None,
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
            require_clean_hashes: true,
            git_head_precondition: false,
            backup_suffix: ".buildfix.bak".to_string(),
            mode: RunMode::Standalone,
        }
    }

    fn make_apply_settings(root: &Utf8Path, out_dir: &Utf8Path) -> ApplySettings {
        ApplySettings {
            repo_root: root.to_path_buf(),
            out_dir: out_dir.to_path_buf(),
            dry_run: true,
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            params: HashMap::new(),
            backup_enabled: false,
            backup_suffix: ".buildfix.bak".to_string(),
            mode: RunMode::Standalone,
        }
    }

    #[test]
    fn report_plan_data_contains_plan_available() {
        let plan = make_plan(
            vec![make_op(SafetyClass::Safe, false, None)],
            Some(SafetyCounts {
                safe: 1,
                guarded: 0,
                unsafe_count: 0,
            }),
        );

        let report = report_from_plan(&plan, tool(), &[]);
        let data = report.data.unwrap();
        let plan_data = &data["buildfix"]["plan"];

        assert_eq!(plan_data["plan_available"], serde_json::json!(true));
    }

    #[test]
    fn report_plan_data_plan_available_false_when_empty() {
        let plan = make_plan(vec![], None);

        let report = report_from_plan(&plan, tool(), &[]);
        let data = report.data.unwrap();
        let plan_data = &data["buildfix"]["plan"];

        assert_eq!(plan_data["plan_available"], serde_json::json!(false));
    }

    #[test]
    fn report_plan_data_contains_safety_counts() {
        let sc = SafetyCounts {
            safe: 2,
            guarded: 1,
            unsafe_count: 0,
        };
        let plan = make_plan(
            vec![
                make_op(SafetyClass::Safe, false, None),
                make_op(SafetyClass::Safe, false, None),
                make_op(SafetyClass::Guarded, false, None),
            ],
            Some(sc),
        );

        let report = report_from_plan(&plan, tool(), &[]);
        let data = report.data.unwrap();
        let plan_data = &data["buildfix"]["plan"];

        let sc_data = &plan_data["safety_counts"];
        assert_eq!(sc_data["safe"], serde_json::json!(2));
        assert_eq!(sc_data["guarded"], serde_json::json!(1));
        assert_eq!(sc_data["unsafe"], serde_json::json!(0));
    }

    #[test]
    fn report_plan_data_contains_blocked_reason_tokens_top() {
        let plan = make_plan(
            vec![
                make_op_with_token(
                    SafetyClass::Safe,
                    true,
                    Some("denied by policy"),
                    Some("denylist"),
                ),
                make_op_with_token(
                    SafetyClass::Guarded,
                    true,
                    Some("missing params: version"),
                    Some("missing_params"),
                ),
            ],
            Some(SafetyCounts {
                safe: 1,
                guarded: 1,
                unsafe_count: 0,
            }),
        );

        let report = report_from_plan(&plan, tool(), &[]);
        let data = report.data.unwrap();
        let plan_data = &data["buildfix"]["plan"];

        let tokens = plan_data["blocked_reason_tokens_top"].as_array().unwrap();
        assert_eq!(tokens.len(), 2);
        // BTreeSet sorts: "denylist" < "missing_params"
        assert_eq!(tokens[0], "denylist");
        assert_eq!(tokens[1], "missing_params");
    }

    #[test]
    fn report_plan_data_no_blocked_reason_tokens_when_none_blocked() {
        let plan = make_plan(
            vec![make_op(SafetyClass::Safe, false, None)],
            Some(SafetyCounts {
                safe: 1,
                guarded: 0,
                unsafe_count: 0,
            }),
        );

        let report = report_from_plan(&plan, tool(), &[]);
        let data = report.data.unwrap();
        let plan_data = &data["buildfix"]["plan"];

        assert!(plan_data.get("blocked_reason_tokens_top").is_none());
    }

    #[test]
    fn report_plan_data_contains_ops_applicable_and_fix_available() {
        let plan = make_plan(
            vec![
                make_op(SafetyClass::Safe, false, None),
                make_op_with_token(SafetyClass::Safe, true, Some("denied"), Some("denylist")),
            ],
            Some(SafetyCounts {
                safe: 2,
                guarded: 0,
                unsafe_count: 0,
            }),
        );

        let report = report_from_plan(&plan, tool(), &[]);
        let data = report.data.unwrap();
        let plan_data = &data["buildfix"]["plan"];

        assert_eq!(plan_data["ops_applicable"], serde_json::json!(1));
        assert_eq!(plan_data["fix_available"], serde_json::json!(true));
    }

    #[test]
    fn report_plan_data_fix_available_false_when_all_blocked() {
        let plan = make_plan(
            vec![make_op_with_token(
                SafetyClass::Safe,
                true,
                Some("denied"),
                Some("denylist"),
            )],
            Some(SafetyCounts {
                safe: 1,
                guarded: 0,
                unsafe_count: 0,
            }),
        );

        let report = report_from_plan(&plan, tool(), &[]);
        let data = report.data.unwrap();
        let plan_data = &data["buildfix"]["plan"];

        assert_eq!(plan_data["ops_applicable"], serde_json::json!(0));
        assert_eq!(plan_data["fix_available"], serde_json::json!(false));
    }

    #[test]
    fn report_apply_data_contains_apply_performed() {
        let mut apply = BuildfixApply::new(
            tool(),
            buildfix_types::apply::ApplyRepoInfo {
                root: ".".into(),
                head_sha_before: None,
                head_sha_after: None,
                dirty_before: None,
                dirty_after: None,
            },
            buildfix_types::apply::PlanRef {
                path: "plan.json".into(),
                sha256: None,
            },
        );
        apply.summary.applied = 3;

        let report = report_from_apply(&apply, tool());
        let data = report.data.unwrap();
        let apply_data = &data["buildfix"]["apply"];

        assert_eq!(apply_data["apply_performed"], serde_json::json!(true));
    }

    #[test]
    fn report_apply_data_apply_performed_false_when_zero() {
        let apply = BuildfixApply::new(
            tool(),
            buildfix_types::apply::ApplyRepoInfo {
                root: ".".into(),
                head_sha_before: None,
                head_sha_after: None,
                dirty_before: None,
                dirty_after: None,
            },
            buildfix_types::apply::PlanRef {
                path: "plan.json".into(),
                sha256: None,
            },
        );

        let report = report_from_apply(&apply, tool());
        let data = report.data.unwrap();
        let apply_data = &data["buildfix"]["apply"];

        assert_eq!(apply_data["apply_performed"], serde_json::json!(false));
    }

    #[test]
    fn report_from_plan_includes_input_failures_and_warn_status() {
        let plan = make_plan(vec![], None);
        let receipts = vec![LoadedReceipt {
            path: Utf8PathBuf::from("artifacts/bad/report.json"),
            sensor_id: "bad".to_string(),
            receipt: Err(ReceiptLoadError::Io {
                message: "missing".to_string(),
            }),
        }];

        let report = report_from_plan(&plan, tool(), &receipts);
        assert_eq!(report.verdict.status, ReportStatus::Warn);
        assert_eq!(report.findings.len(), 1);
        assert!(report.findings[0]
            .message
            .contains("Receipt failed to load"));
        assert!(report
            .capabilities
            .as_ref()
            .unwrap()
            .inputs_failed
            .iter()
            .any(|f| f.path.contains("report.json")));
    }

    #[test]
    fn report_from_apply_sets_status_for_failed_and_blocked() {
        let mut apply = BuildfixApply::new(
            tool(),
            buildfix_types::apply::ApplyRepoInfo {
                root: ".".into(),
                head_sha_before: None,
                head_sha_after: None,
                dirty_before: None,
                dirty_after: None,
            },
            buildfix_types::apply::PlanRef {
                path: "plan.json".into(),
                sha256: None,
            },
        );

        apply.summary.failed = 1;
        let report = report_from_apply(&apply, tool());
        assert_eq!(report.verdict.status, ReportStatus::Fail);

        apply.summary.failed = 0;
        apply.summary.blocked = 1;
        let report = report_from_apply(&apply, tool());
        assert_eq!(report.verdict.status, ReportStatus::Warn);

        apply.summary.blocked = 0;
        apply.summary.applied = 1;
        let report = report_from_apply(&apply, tool());
        assert_eq!(report.verdict.status, ReportStatus::Pass);

        apply.summary.applied = 0;
        let report = report_from_apply(&apply, tool());
        assert_eq!(report.verdict.status, ReportStatus::Warn);
    }

    #[test]
    fn run_plan_attaches_preconditions_and_git_info() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let receipts = crate::adapters::InMemoryReceiptSource::new(vec![resolver_receipt()]);

        let mut settings = build_plan_settings(&root);
        settings.git_head_precondition = true;

        let git = StubGitPort {
            head: Some("deadbeef".to_string()),
            dirty: Some(true),
        };

        let outcome = run_plan(&settings, &receipts, &git, tool()).expect("run_plan");
        assert_eq!(outcome.plan.ops.len(), 1);

        assert_eq!(outcome.plan.preconditions.files.len(), 1);
        let pre = &outcome.plan.preconditions.files[0];
        assert_eq!(pre.path, "Cargo.toml");

        let mut hasher = Sha256::new();
        hasher.update(b"[workspace]\nresolver = \"1\"\n");
        let expected_sha = hex::encode(hasher.finalize());
        assert_eq!(pre.sha256, expected_sha);

        assert_eq!(outcome.plan.preconditions.head_sha.as_deref(), Some("deadbeef"));
        assert_eq!(outcome.plan.preconditions.dirty, Some(true));
        assert_eq!(outcome.plan.repo.head_sha.as_deref(), Some("deadbeef"));
        assert_eq!(outcome.plan.repo.dirty, Some(true));
    }

    #[test]
    fn run_plan_skips_file_preconditions_when_disabled() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let receipts = crate::adapters::InMemoryReceiptSource::new(vec![resolver_receipt()]);

        let mut settings = build_plan_settings(&root);
        settings.require_clean_hashes = false;
        settings.git_head_precondition = true;

        let git = StubGitPort {
            head: Some("cafebabe".to_string()),
            dirty: Some(false),
        };

        let outcome = run_plan(&settings, &receipts, &git, tool()).expect("run_plan");
        assert!(outcome.plan.preconditions.files.is_empty());
        assert_eq!(outcome.plan.preconditions.head_sha.as_deref(), Some("cafebabe"));
        assert_eq!(outcome.plan.preconditions.dirty, Some(false));
    }

    #[test]
    fn run_plan_blocks_when_patch_cap_exceeded() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let receipts = crate::adapters::InMemoryReceiptSource::new(vec![resolver_receipt()]);

        let mut settings = build_plan_settings(&root);
        settings.max_patch_bytes = Some(0);

        let git = StubGitPort::default();
        let outcome = run_plan(&settings, &receipts, &git, tool()).expect("run_plan");

        assert!(outcome.plan.ops.iter().all(|o| o.blocked));
        assert_eq!(outcome.plan.summary.ops_blocked, outcome.plan.ops.len() as u64);
        assert_eq!(outcome.plan.summary.patch_bytes, Some(0));
        assert!(outcome.patch.is_empty());
        assert!(outcome.policy_block);

        for op in &outcome.plan.ops {
            assert_eq!(
                op.blocked_reason_token.as_deref(),
                Some(buildfix_types::plan::blocked_tokens::MAX_PATCH_BYTES)
            );
        }
    }

    #[test]
    fn write_plan_artifacts_writes_expected_files() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let receipts = crate::adapters::InMemoryReceiptSource::new(vec![resolver_receipt()]);
        let settings = build_plan_settings(&root);
        let git = StubGitPort::default();

        let outcome = run_plan(&settings, &receipts, &git, tool()).expect("run_plan");

        let writer = MemWritePort::default();
        let out_dir = Utf8PathBuf::from("out");
        write_plan_artifacts(&outcome, &out_dir, &writer).expect("write artifacts");

        let files = writer.files.lock().expect("files");
        assert!(files.contains_key("out/plan.json"));
        assert!(files.contains_key("out/plan.md"));
        assert!(files.contains_key("out/comment.md"));
        assert!(files.contains_key("out/patch.diff"));
        assert!(files.contains_key("out/report.json"));
        assert!(files.contains_key("out/extras/buildfix.report.v1.json"));

        let extras = files
            .get("out/extras/buildfix.report.v1.json")
            .expect("extras json");
        let json: serde_json::Value = serde_json::from_slice(extras).expect("parse extras");
        assert_eq!(json["schema"], buildfix_types::schema::BUILDFIX_REPORT_V1);
        assert_eq!(json["artifacts"]["comment"], "comment.md");
    }

    #[test]
    fn run_apply_blocks_on_dirty_working_tree() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).expect("out dir");

        let plan = make_plan(vec![make_op(SafetyClass::Safe, false, None)], None);
        let plan_wire = PlanV1::try_from(&plan).expect("wire");
        let plan_json = serde_json::to_string_pretty(&plan_wire).expect("plan json");
        std::fs::write(out_dir.join("plan.json"), plan_json).expect("write plan");

        let mut settings = make_apply_settings(&root, &out_dir);
        settings.dry_run = false;

        let git = StubGitPort {
            head: Some("deadbeef".to_string()),
            dirty: Some(true),
        };

        let outcome = run_apply(&settings, &git, tool()).expect("run_apply");
        assert!(outcome.policy_block);
        assert_eq!(outcome.apply.summary.blocked, plan.ops.len() as u64);
        assert!(outcome.apply.results.iter().all(|r| r.status == buildfix_types::apply::ApplyStatus::Blocked));
        assert!(!outcome.apply.preconditions.verified);
        assert!(outcome
            .apply
            .preconditions
            .mismatches
            .iter()
            .any(|m| m.path == "<working_tree>"));
        assert!(outcome.patch.is_empty());
        assert!(outcome.apply.plan_ref.sha256.as_deref().unwrap_or("").len() >= 64);
    }

    #[test]
    fn run_apply_parses_raw_plan_json_and_runs_dry_run() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).expect("out dir");

        let tool_no_version = ToolInfo {
            name: "buildfix".to_string(),
            version: None,
            repo: None,
            commit: None,
        };
        let repo = RepoInfo {
            root: root.to_string(),
            head_sha: None,
            dirty: None,
        };
        let mut plan = BuildfixPlan::new(tool_no_version, repo, PlanPolicy::default());
        plan.ops.push(make_op(SafetyClass::Safe, false, None));
        plan.summary = PlanSummary {
            ops_total: 1,
            ops_blocked: 0,
            files_touched: 1,
            patch_bytes: None,
            safety_counts: None,
        };
        let plan_json = serde_json::to_string_pretty(&plan).expect("plan json");
        std::fs::write(out_dir.join("plan.json"), plan_json).expect("write plan");

        let settings = make_apply_settings(&root, &out_dir);
        let git = StubGitPort::default();

        let outcome = run_apply(&settings, &git, tool()).expect("run_apply");
        assert_eq!(outcome.apply.results.len(), 1);
        assert_eq!(
            outcome.apply.results[0].status,
            buildfix_types::apply::ApplyStatus::Skipped
        );
        assert!(!outcome.patch.is_empty());
        assert!(!outcome.policy_block);
    }

    #[test]
    fn write_apply_artifacts_writes_expected_files() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).expect("out dir");

        let plan = make_plan(vec![make_op(SafetyClass::Safe, false, None)], None);
        let plan_wire = PlanV1::try_from(&plan).expect("wire");
        let plan_json = serde_json::to_string_pretty(&plan_wire).expect("plan json");
        std::fs::write(out_dir.join("plan.json"), plan_json).expect("write plan");

        let settings = make_apply_settings(&root, &out_dir);
        let git = StubGitPort::default();
        let outcome = run_apply(&settings, &git, tool()).expect("run_apply");

        let writer = MemWritePort::default();
        let out_dir = Utf8PathBuf::from("out");
        write_apply_artifacts(&outcome, &out_dir, &writer).expect("write apply artifacts");

        let files = writer.files.lock().expect("files");
        assert!(files.contains_key("out/apply.json"));
        assert!(files.contains_key("out/apply.md"));
        assert!(files.contains_key("out/patch.diff"));
        assert!(files.contains_key("out/report.json"));
        assert!(files.contains_key("out/extras/buildfix.report.v1.json"));

        let extras = files
            .get("out/extras/buildfix.report.v1.json")
            .expect("extras json");
        let json: serde_json::Value = serde_json::from_slice(extras).expect("parse extras");
        assert_eq!(json["schema"], buildfix_types::schema::BUILDFIX_REPORT_V1);
    }
}
