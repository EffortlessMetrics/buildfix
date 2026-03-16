//! Reporting projections for buildfix outcomes.

use chrono::Utc;
use std::collections::BTreeSet;

use buildfix_receipts::LoadedReceipt;
use buildfix_types::apply::BuildfixApply;
use buildfix_types::plan::BuildfixPlan;
use buildfix_types::receipt::ToolInfo;
use buildfix_types::report::{
    BuildfixReport, InputFailure, ReportArtifacts, ReportCapabilities, ReportCounts, ReportFinding,
    ReportRunInfo, ReportSeverity, ReportStatus, ReportToolInfo, ReportVerdict,
};

pub fn build_report_capabilities(receipts: &[LoadedReceipt]) -> ReportCapabilities {
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

pub fn build_plan_report(
    plan: &BuildfixPlan,
    tool: ToolInfo,
    receipts: &[LoadedReceipt],
) -> BuildfixReport {
    let capabilities = build_report_capabilities(receipts);
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

    let findings: Vec<ReportFinding> = capabilities
        .inputs_failed
        .iter()
        .map(|failure| ReportFinding {
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
        })
        .collect();

    let warn_count = plan.ops.len() as u64 + capabilities.inputs_failed.len() as u64;
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
        data: Some(serde_json::json!({
            "buildfix": {
                "plan": plan_data
            }
        })),
    }
}

pub fn build_apply_report(apply: &BuildfixApply, tool: ToolInfo) -> BuildfixReport {
    let status = if apply.summary.failed > 0 {
        ReportStatus::Fail
    } else if apply.summary.blocked > 0 {
        ReportStatus::Warn
    } else if apply.summary.applied > 0 {
        ReportStatus::Pass
    } else {
        ReportStatus::Warn
    };

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
        data: Some(serde_json::json!({
            "buildfix": {
                "apply": apply_data
            }
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buildfix_receipts::{LoadedReceipt, ReceiptLoadError};
    use buildfix_types::{
        apply::{ApplyRepoInfo, BuildfixApply, PlanRef},
        plan::{BuildfixPlan, PlanPolicy, PlanSummary},
        receipt::{Finding, ReceiptCapabilities, ReceiptEnvelope, RunInfo, ToolInfo, Verdict},
    };
    use chrono::Utc;

    fn fixture_tool() -> ToolInfo {
        ToolInfo {
            name: "buildfix".to_string(),
            version: Some("0.0.0".to_string()),
            repo: None,
            commit: None,
        }
    }

    #[test]
    fn capabilities_are_sorted_and_deduplicated() {
        let receipts = vec![
            LoadedReceipt {
                path: "artifacts/second/report.json".into(),
                sensor_id: "second".to_string(),
                receipt: Ok(ReceiptEnvelope {
                    schema: "sensor.report.v1".to_string(),
                    tool: fixture_tool(),
                    run: RunInfo {
                        started_at: Some(Utc::now()),
                        ended_at: Some(Utc::now()),
                        git_head_sha: None,
                    },
                    verdict: Verdict::default(),
                    findings: vec![Finding {
                        severity: Default::default(),
                        check_id: Some("b.check".to_string()),
                        code: None,
                        message: None,
                        location: None,
                        fingerprint: None,
                        data: None,
                    }],
                    capabilities: Some(ReceiptCapabilities {
                        check_ids: vec!["z.check".to_string(), "a.check".to_string()],
                        scopes: vec!["workspace".to_string(), "crate".to_string()],
                        partial: false,
                        reason: None,
                    }),
                    data: None,
                }),
            },
            LoadedReceipt {
                path: "artifacts/first/report.json".into(),
                sensor_id: "first".to_string(),
                receipt: Ok(ReceiptEnvelope {
                    schema: "sensor.report.v1".to_string(),
                    tool: fixture_tool(),
                    run: RunInfo {
                        started_at: Some(Utc::now()),
                        ended_at: Some(Utc::now()),
                        git_head_sha: None,
                    },
                    verdict: Verdict::default(),
                    findings: vec![Finding {
                        severity: Default::default(),
                        check_id: Some("a.check".to_string()),
                        code: None,
                        message: None,
                        location: None,
                        fingerprint: None,
                        data: None,
                    }],
                    capabilities: None,
                    data: None,
                }),
            },
            LoadedReceipt {
                path: "artifacts/error/report.json".into(),
                sensor_id: "err".to_string(),
                receipt: Err(ReceiptLoadError::Io {
                    message: "boom".to_string(),
                }),
            },
        ];

        let caps = build_report_capabilities(&receipts);
        assert_eq!(
            caps.check_ids,
            vec![
                "a.check".to_string(),
                "b.check".to_string(),
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
                "artifacts/first/report.json".to_string(),
                "artifacts/second/report.json".to_string(),
            ]
        );
        assert!(caps.partial);
        assert_eq!(caps.inputs_failed.len(), 1);
    }

    #[test]
    fn plan_report_marks_warning_when_inputs_fail() {
        let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
        plan.summary = PlanSummary {
            ops_total: 0,
            ops_blocked: 0,
            files_touched: 0,
            patch_bytes: None,
            safety_counts: None,
        };

        let report = build_plan_report(
            &plan,
            fixture_tool(),
            &[LoadedReceipt {
                path: "artifacts/bad/report.json".into(),
                sensor_id: "bad".to_string(),
                receipt: Err(ReceiptLoadError::Io {
                    message: "missing".to_string(),
                }),
            }],
        );

        assert_eq!(
            report.verdict.status,
            buildfix_types::report::ReportStatus::Warn
        );
        assert_eq!(report.findings[0].code, "receipt_load_failed");
    }

    #[test]
    fn apply_report_status_rules() {
        let mut apply = BuildfixApply::new(
            fixture_tool(),
            ApplyRepoInfo {
                root: ".".to_string(),
                head_sha_before: None,
                head_sha_after: None,
                dirty_before: None,
                dirty_after: None,
            },
            PlanRef {
                path: "plan.json".into(),
                sha256: None,
            },
        );

        assert_eq!(
            build_apply_report(&apply, fixture_tool()).verdict.status,
            buildfix_types::report::ReportStatus::Warn
        );
        apply.summary.failed = 1;
        assert_eq!(
            build_apply_report(&apply, fixture_tool()).verdict.status,
            buildfix_types::report::ReportStatus::Fail
        );
        apply.summary.failed = 0;
        apply.summary.blocked = 1;
        assert_eq!(
            build_apply_report(&apply, fixture_tool()).verdict.status,
            buildfix_types::report::ReportStatus::Warn
        );
        apply.summary.blocked = 0;
        apply.summary.applied = 1;
        assert_eq!(
            build_apply_report(&apply, fixture_tool()).verdict.status,
            buildfix_types::report::ReportStatus::Pass
        );
    }

    fn default_repo() -> buildfix_types::plan::RepoInfo {
        buildfix_types::plan::RepoInfo {
            root: ".".to_string(),
            head_sha: None,
            dirty: None,
        }
    }
}
