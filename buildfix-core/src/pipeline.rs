//! Core plan and apply pipelines, extracted from the CLI.
//!
//! These entry points are I/O-agnostic: all filesystem and git operations
//! are performed through the port traits.

use crate::ports::{GitPort, ReceiptSource, WritePort};
use crate::settings::{ApplySettings, PlanSettings};
use anyhow::Context;
use buildfix_artifacts::{
    ArtifactWriter, write_apply_artifacts as write_apply_artifacts_io,
    write_plan_artifacts as write_plan_artifacts_io,
};
use buildfix_domain::{FsRepoView, PlanContext, Planner, PlannerConfig};
use buildfix_edit::{
    ApplyOptions, AttachPreconditionsOptions, apply_plan, attach_preconditions, preview_patch,
};
use buildfix_hash::sha256_hex;
use buildfix_receipts::LoadedReceipt;
#[cfg(feature = "reporting")]
use buildfix_report::{build_apply_report, build_plan_report};
use buildfix_types::apply::{AutoCommitInfo, BuildfixApply};
use buildfix_types::plan::BuildfixPlan;
use buildfix_types::receipt::ToolInfo;
use buildfix_types::report::BuildfixReport;
#[cfg(not(feature = "reporting"))]
use buildfix_types::report::{
    InputFailure, ReportArtifacts, ReportCapabilities, ReportCounts, ReportFinding, ReportRunInfo,
    ReportSeverity, ReportStatus, ReportToolInfo, ReportVerdict,
};
use buildfix_types::wire::PlanV1;
#[cfg(not(feature = "reporting"))]
use chrono::Utc;
#[cfg(not(feature = "reporting"))]
use std::collections::BTreeSet;
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
#[cfg(feature = "artifact-writer")]
pub fn write_plan_artifacts(
    outcome: &PlanOutcome,
    out_dir: &camino::Utf8Path,
    writer: &dyn WritePort,
) -> anyhow::Result<()> {
    let adapter = CoreArtifactWriter { writer };
    write_plan_artifacts_io(
        &outcome.plan,
        &outcome.report,
        &outcome.patch,
        out_dir,
        &adapter,
    )
}

#[cfg(not(feature = "artifact-writer"))]
pub fn write_plan_artifacts(
    _outcome: &PlanOutcome,
    _out_dir: &camino::Utf8Path,
    _writer: &dyn WritePort,
) -> anyhow::Result<()> {
    anyhow::bail!("artifact-writer feature is disabled for buildfix-core")
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
    let plan_sha = sha256_hex(plan_str.as_bytes());

    let plan: BuildfixPlan = match serde_json::from_str::<PlanV1>(&plan_str) {
        Ok(wire) => BuildfixPlan::from(wire),
        Err(err) => {
            debug!("plan.json is not wire format: {}", err);
            serde_json::from_str(&plan_str).context("parse plan.json")?
        }
    };

    let head_before = git.head_sha(&settings.repo_root).ok().flatten();
    let dirty_before = git.is_dirty(&settings.repo_root).ok().flatten();

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
    let mut dirty_block_message = "dirty working tree".to_string();

    // Block apply on dirty working tree unless explicitly allowed.
    if !settings.dry_run && !settings.allow_dirty && dirty_before == Some(true) {
        policy_block_dirty = true;
    }

    // Auto-commit is maintainer-only and requires a known-clean git tree.
    if settings.auto_commit && !settings.dry_run && dirty_before != Some(false) {
        policy_block_dirty = true;
        dirty_block_message = "auto-commit requires clean git working tree".to_string();
    }

    let (mut apply, patch) = if policy_block_dirty {
        let mut apply = empty_apply_from_plan(&plan, &settings.repo_root, tool.clone(), &plan_path);
        let dirty_actual = match dirty_before {
            Some(true) => "dirty".to_string(),
            Some(false) => "clean".to_string(),
            None => "unknown".to_string(),
        };
        apply.preconditions.verified = false;
        apply
            .preconditions
            .mismatches
            .push(buildfix_types::apply::PreconditionMismatch {
                path: "<working_tree>".to_string(),
                expected: "clean".to_string(),
                actual: dirty_actual,
            });
        for op in &plan.ops {
            apply.results.push(buildfix_types::apply::ApplyResult {
                op_id: op.id.clone(),
                status: buildfix_types::apply::ApplyStatus::Blocked,
                message: Some(dirty_block_message.clone()),
                blocked_reason: Some(dirty_block_message.clone()),
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
        sha256: Some(plan_sha.clone()),
    };
    apply.repo = buildfix_types::apply::ApplyRepoInfo {
        root: settings.repo_root.to_string(),
        head_sha_before: head_before.clone(),
        head_sha_after: head_before,
        dirty_before,
        dirty_after: dirty_before,
    };

    if settings.auto_commit {
        let mut auto_commit = AutoCommitInfo {
            enabled: true,
            attempted: false,
            committed: false,
            commit_sha: None,
            message: settings.commit_message.clone(),
            skip_reason: None,
        };

        if settings.dry_run {
            auto_commit.skip_reason = Some("dry-run: auto-commit skipped".to_string());
        } else if apply.summary.applied == 0 {
            auto_commit.skip_reason = Some("no applied ops to commit".to_string());
        } else if apply.summary.blocked > 0
            || apply.summary.failed > 0
            || !apply.preconditions.verified
        {
            auto_commit.skip_reason =
                Some("apply not fully successful; skipping auto-commit".to_string());
        } else {
            auto_commit.attempted = true;
            let message = settings
                .commit_message
                .clone()
                .unwrap_or_else(|| default_auto_commit_message(&plan_path, &plan_sha, &apply));
            auto_commit.message = Some(message.clone());

            match git.commit_all(&settings.repo_root, &message) {
                Ok(Some(commit_sha)) => {
                    auto_commit.committed = true;
                    auto_commit.commit_sha = Some(commit_sha.clone());
                    apply.repo.head_sha_after = Some(commit_sha);
                }
                Ok(None) => {
                    auto_commit.skip_reason = Some("no changes were committed".to_string());
                }
                Err(err) => {
                    return Err(ToolError::Internal(anyhow::anyhow!(
                        "auto-commit failed: {}",
                        err
                    )));
                }
            }
        }

        apply.auto_commit = Some(auto_commit);
    }

    apply.repo.dirty_after = git.is_dirty(&settings.repo_root).ok().flatten();
    if apply.repo.head_sha_after.is_none() {
        apply.repo.head_sha_after = git.head_sha(&settings.repo_root).ok().flatten();
    }

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
#[cfg(feature = "artifact-writer")]
pub fn write_apply_artifacts(
    outcome: &ApplyOutcome,
    out_dir: &camino::Utf8Path,
    writer: &dyn WritePort,
) -> anyhow::Result<()> {
    let adapter = CoreArtifactWriter { writer };
    write_apply_artifacts_io(
        &outcome.apply,
        &outcome.report,
        &outcome.patch,
        out_dir,
        &adapter,
    )
}

struct CoreArtifactWriter<'a> {
    writer: &'a dyn WritePort,
}

impl<'a> ArtifactWriter for CoreArtifactWriter<'a> {
    fn write_file(&self, path: &camino::Utf8Path, contents: &[u8]) -> anyhow::Result<()> {
        self.writer.write_file(path, contents)
    }

    fn create_dir_all(&self, path: &camino::Utf8Path) -> anyhow::Result<()> {
        self.writer.create_dir_all(path)
    }
}

#[cfg(not(feature = "artifact-writer"))]
pub fn write_apply_artifacts(
    _outcome: &ApplyOutcome,
    _out_dir: &camino::Utf8Path,
    _writer: &dyn WritePort,
) -> anyhow::Result<()> {
    anyhow::bail!("artifact-writer feature is disabled for buildfix-core")
}

#[cfg(feature = "reporting")]
pub(crate) fn report_from_plan(
    plan: &BuildfixPlan,
    tool: ToolInfo,
    receipts: &[LoadedReceipt],
) -> BuildfixReport {
    build_plan_report(plan, tool, receipts)
}

#[cfg(feature = "reporting")]
pub(crate) fn report_from_apply(apply: &BuildfixApply, tool: ToolInfo) -> BuildfixReport {
    build_apply_report(apply, tool)
}

#[cfg(not(feature = "reporting"))]
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
            git_head_sha: plan.repo.head_sha.clone(),
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
            comment: Some("comment.md".to_string()),
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
            let tokens: BTreeSet<&str> = plan
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

#[cfg(not(feature = "reporting"))]
fn build_capabilities(receipts: &[LoadedReceipt]) -> ReportCapabilities {
    let mut inputs_available = Vec::new();
    let mut inputs_failed = Vec::new();
    let mut check_ids = BTreeSet::new();
    let mut scopes = BTreeSet::new();

    for r in receipts {
        match &r.receipt {
            Ok(receipt) => {
                inputs_available.push(r.path.to_string());
                if let Some(caps) = &receipt.capabilities {
                    check_ids.extend(caps.check_ids.iter().cloned());
                    scopes.extend(caps.scopes.iter().cloned());
                }
                for finding in &receipt.findings {
                    if let Some(check_id) = finding.check_id.as_ref()
                        && !check_id.is_empty()
                    {
                        check_ids.insert(check_id.clone());
                    }
                }
            }
            Err(e) => {
                inputs_failed.push(InputFailure {
                    path: r.path.to_string(),
                    reason: e.to_string(),
                });
            }
        }
    }

    inputs_available.sort();
    inputs_failed.sort_by(|a, b| a.path.cmp(&b.path));

    ReportCapabilities {
        check_ids: check_ids.into_iter().collect(),
        scopes: scopes.into_iter().collect(),
        partial: !inputs_failed.is_empty(),
        reason: if !inputs_failed.is_empty() {
            Some("some receipts failed to load".to_string())
        } else {
            None
        },
        inputs_available,
        inputs_failed,
    }
}

#[cfg(not(feature = "reporting"))]
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
            git_head_sha: apply.repo.head_sha_after.clone(),
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
        data: Some({
            let mut apply_data = serde_json::json!({
                "attempted": apply.summary.attempted,
                "applied": apply.summary.applied,
                "blocked": apply.summary.blocked,
                "failed": apply.summary.failed,
                "files_modified": apply.summary.files_modified,
                "apply_performed": apply.summary.applied > 0,
            });
            if let Some(auto_commit) = &apply.auto_commit {
                apply_data["auto_commit"] = serde_json::json!({
                    "enabled": auto_commit.enabled,
                    "attempted": auto_commit.attempted,
                    "committed": auto_commit.committed,
                    "commit_sha": auto_commit.commit_sha,
                    "message": auto_commit.message,
                    "skip_reason": auto_commit.skip_reason,
                });
            }

            serde_json::json!({
                "buildfix": {
                    "apply": apply_data
                }
            })
        }),
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

fn default_auto_commit_message(
    plan_path: &camino::Utf8Path,
    plan_sha: &str,
    apply: &BuildfixApply,
) -> String {
    let short_sha = if plan_sha.len() >= 12 {
        &plan_sha[..12]
    } else {
        plan_sha
    };
    format!(
        "buildfix: apply plan {}\n\nplan={}\nops_applied={}\nfiles_modified={}",
        short_sha, plan_path, apply.summary.applied, apply.summary.files_modified
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::RunMode;
    use buildfix_receipts::{LoadedReceipt, ReceiptLoadError};
    use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
    use buildfix_types::plan::{
        PlanOp, PlanPolicy, PlanSummary, Rationale, RepoInfo, SafetyCounts,
    };
    use buildfix_types::receipt::{
        Finding, Location, ReceiptCapabilities, ReceiptEnvelope, RunInfo, ToolInfo, Verdict,
    };
    use buildfix_types::report::ReportStatus;
    use buildfix_types::wire::PlanV1;
    use camino::{Utf8Path, Utf8PathBuf};
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

    struct CommitGitPort {
        head_before: Option<String>,
        head_after: Option<String>,
        dirty_before: Option<bool>,
        dirty_after: Option<bool>,
        commit_sha: Option<String>,
        commit_calls: Mutex<u64>,
    }

    impl Default for CommitGitPort {
        fn default() -> Self {
            Self {
                head_before: None,
                head_after: None,
                dirty_before: Some(false),
                dirty_after: Some(false),
                commit_sha: None,
                commit_calls: Mutex::new(0),
            }
        }
    }

    impl GitPort for CommitGitPort {
        fn head_sha(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<String>> {
            let committed = *self.commit_calls.lock().expect("commit calls") > 0;
            if committed {
                Ok(self.head_after.clone())
            } else {
                Ok(self.head_before.clone())
            }
        }

        fn is_dirty(&self, _repo_root: &Utf8Path) -> anyhow::Result<Option<bool>> {
            let committed = *self.commit_calls.lock().expect("commit calls") > 0;
            if committed {
                Ok(self.dirty_after)
            } else {
                Ok(self.dirty_before)
            }
        }

        fn commit_all(
            &self,
            _repo_root: &Utf8Path,
            _message: &str,
        ) -> anyhow::Result<Option<String>> {
            let mut calls = self.commit_calls.lock().expect("commit calls");
            *calls += 1;
            Ok(self.commit_sha.clone())
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

    struct FailingReceiptSource;

    impl ReceiptSource for FailingReceiptSource {
        fn load_receipts(&self) -> anyhow::Result<Vec<LoadedReceipt>> {
            Err(anyhow::anyhow!("receipt load failed"))
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
            auto_commit: false,
            commit_message: None,
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
    fn report_from_plan_passes_when_no_ops_and_no_failures() {
        let plan = make_plan(vec![], None);
        let report = report_from_plan(&plan, tool(), &[]);
        assert_eq!(report.verdict.status, ReportStatus::Pass);
        assert_eq!(report.verdict.counts.warn, 0);
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
    fn report_from_plan_uses_unknown_version_when_missing() {
        let plan = make_plan(vec![], None);
        let mut t = tool();
        t.version = None;
        let report = report_from_plan(&plan, t, &[]);
        assert_eq!(report.tool.version, "unknown");
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
    fn report_apply_data_includes_auto_commit_when_present() {
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
        apply.auto_commit = Some(buildfix_types::apply::AutoCommitInfo {
            enabled: true,
            attempted: true,
            committed: true,
            commit_sha: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            message: Some("msg".to_string()),
            skip_reason: None,
        });

        let report = report_from_apply(&apply, tool());
        let data = report.data.unwrap();
        let auto_commit = &data["buildfix"]["apply"]["auto_commit"];

        assert_eq!(auto_commit["enabled"], serde_json::json!(true));
        assert_eq!(auto_commit["attempted"], serde_json::json!(true));
        assert_eq!(auto_commit["committed"], serde_json::json!(true));
        assert_eq!(
            auto_commit["commit_sha"],
            serde_json::json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
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
        assert!(
            report.findings[0]
                .message
                .contains("Receipt failed to load")
        );
        assert!(
            report
                .capabilities
                .as_ref()
                .unwrap()
                .inputs_failed
                .iter()
                .any(|f| f.path.contains("report.json"))
        );
    }

    #[test]
    fn report_from_plan_collects_check_ids_scopes_and_sorts_inputs() {
        let plan = make_plan(vec![], None);
        let receipt_with_caps = ReceiptEnvelope {
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
                code: None,
                message: None,
                location: None,
                fingerprint: None,
                data: None,
            }],
            capabilities: Some(ReceiptCapabilities {
                check_ids: vec![
                    "z.check".to_string(),
                    "a.check".to_string(),
                    "workspace.resolver_v2".to_string(),
                ],
                scopes: vec!["workspace".to_string(), "crate".to_string()],
                partial: false,
                reason: None,
            }),
            data: None,
        };
        let receipt_findings_only = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: ToolInfo {
                name: "depguard".to_string(),
                version: Some("1.0.0".to_string()),
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![
                Finding {
                    severity: Default::default(),
                    check_id: Some("b.check".to_string()),
                    code: None,
                    message: None,
                    location: None,
                    fingerprint: None,
                    data: None,
                },
                Finding {
                    severity: Default::default(),
                    check_id: Some(String::new()),
                    code: None,
                    message: None,
                    location: None,
                    fingerprint: None,
                    data: None,
                },
            ],
            capabilities: None,
            data: None,
        };
        let receipts = vec![
            LoadedReceipt {
                path: Utf8PathBuf::from("artifacts/z/report.json"),
                sensor_id: "z".to_string(),
                receipt: Ok(receipt_findings_only),
            },
            LoadedReceipt {
                path: Utf8PathBuf::from("artifacts/a/report.json"),
                sensor_id: "a".to_string(),
                receipt: Ok(receipt_with_caps),
            },
        ];

        let report = report_from_plan(&plan, tool(), &receipts);
        let caps = report.capabilities.expect("capabilities");

        assert_eq!(
            caps.check_ids,
            vec![
                "a.check".to_string(),
                "b.check".to_string(),
                "workspace.resolver_v2".to_string(),
                "z.check".to_string(),
            ]
        );
        assert_eq!(
            caps.scopes,
            vec!["crate".to_string(), "workspace".to_string()]
        );
        assert_eq!(
            caps.inputs_available,
            vec![
                "artifacts/a/report.json".to_string(),
                "artifacts/z/report.json".to_string(),
            ]
        );
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

        let expected_sha = sha256_hex(b"[workspace]\nresolver = \"1\"\n");
        assert_eq!(pre.sha256, expected_sha);

        assert_eq!(
            outcome.plan.preconditions.head_sha.as_deref(),
            Some("deadbeef")
        );
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
        assert_eq!(
            outcome.plan.preconditions.head_sha.as_deref(),
            Some("cafebabe")
        );
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
        assert_eq!(
            outcome.plan.summary.ops_blocked,
            outcome.plan.ops.len() as u64
        );
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
    fn run_plan_propagates_receipt_load_errors() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let settings = build_plan_settings(&root);
        let git = StubGitPort::default();

        let err = run_plan(&settings, &FailingReceiptSource, &git, tool())
            .err()
            .expect("run_plan");
        match err {
            ToolError::Internal(e) => {
                assert!(e.to_string().contains("receipt load failed"));
            }
            ToolError::PolicyBlock => panic!("expected internal error"),
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
        assert!(
            outcome
                .apply
                .results
                .iter()
                .all(|r| r.status == buildfix_types::apply::ApplyStatus::Blocked)
        );
        assert!(!outcome.apply.preconditions.verified);
        assert!(
            outcome
                .apply
                .preconditions
                .mismatches
                .iter()
                .any(|m| m.path == "<working_tree>")
        );
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
    fn run_apply_auto_commit_updates_head_and_metadata() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).expect("out dir");

        let plan = make_plan(vec![make_op(SafetyClass::Safe, false, None)], None);
        let plan_wire = PlanV1::try_from(&plan).expect("wire");
        let plan_json = serde_json::to_string_pretty(&plan_wire).expect("plan json");
        std::fs::write(out_dir.join("plan.json"), plan_json).expect("write plan");

        let mut settings = make_apply_settings(&root, &out_dir);
        settings.dry_run = false;
        settings.auto_commit = true;

        let git = CommitGitPort {
            head_before: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
            head_after: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
            dirty_before: Some(false),
            dirty_after: Some(false),
            commit_sha: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
            commit_calls: Mutex::new(0),
        };

        let outcome = run_apply(&settings, &git, tool()).expect("run_apply");
        assert!(!outcome.policy_block);
        assert_eq!(outcome.apply.summary.applied, 1);
        assert_eq!(
            outcome.apply.repo.head_sha_after.as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        assert!(outcome.apply.auto_commit.is_some());
        let auto_commit = outcome.apply.auto_commit.as_ref().expect("auto_commit");
        assert!(auto_commit.enabled);
        assert!(auto_commit.attempted);
        assert!(auto_commit.committed);
        assert_eq!(
            auto_commit.commit_sha.as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
    }

    #[test]
    fn run_apply_auto_commit_blocks_when_tree_is_dirty() {
        let (_temp, root) = create_temp_repo("[workspace]\nresolver = \"1\"\n");
        let out_dir = root.join("artifacts").join("buildfix");
        std::fs::create_dir_all(&out_dir).expect("out dir");

        let plan = make_plan(vec![make_op(SafetyClass::Safe, false, None)], None);
        let plan_wire = PlanV1::try_from(&plan).expect("wire");
        let plan_json = serde_json::to_string_pretty(&plan_wire).expect("plan json");
        std::fs::write(out_dir.join("plan.json"), plan_json).expect("write plan");

        let mut settings = make_apply_settings(&root, &out_dir);
        settings.dry_run = false;
        settings.allow_dirty = true;
        settings.auto_commit = true;

        let git = StubGitPort {
            head: Some("deadbeef".to_string()),
            dirty: Some(true),
        };

        let outcome = run_apply(&settings, &git, tool()).expect("run_apply");
        assert!(outcome.policy_block);
        assert_eq!(outcome.apply.summary.blocked, 1);
        assert!(
            outcome
                .apply
                .results
                .iter()
                .all(|r| r.blocked_reason.as_deref()
                    == Some("auto-commit requires clean git working tree"))
        );
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
