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
        apply::{ApplyRepoInfo, AutoCommitInfo, BuildfixApply, PlanRef},
        ops::{OpKind, OpTarget, SafetyClass},
        plan::{BuildfixPlan, PlanOp, PlanPolicy, PlanSummary, Rationale, SafetyCounts},
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
                        ..Default::default()
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
                        ..Default::default()
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

    #[test]
    fn test_capabilities_empty_receipts() {
        let caps = build_report_capabilities(&[]);
        assert!(caps.check_ids.is_empty());
        assert!(caps.scopes.is_empty());
        assert!(!caps.partial);
        assert!(caps.inputs_available.is_empty());
        assert!(caps.inputs_failed.is_empty());
        assert!(caps.reason.is_none());
    }

    #[test]
    fn test_capabilities_all_failed() {
        let receipts = vec![
            LoadedReceipt {
                path: "artifacts/fail1/report.json".into(),
                sensor_id: "fail1".to_string(),
                receipt: Err(ReceiptLoadError::Io {
                    message: "not found".to_string(),
                }),
            },
            LoadedReceipt {
                path: "artifacts/fail2/report.json".into(),
                sensor_id: "fail2".to_string(),
                receipt: Err(ReceiptLoadError::Json {
                    message: "invalid json".to_string(),
                }),
            },
        ];

        let caps = build_report_capabilities(&receipts);
        assert!(caps.partial);
        assert!(caps.inputs_available.is_empty());
        assert_eq!(caps.inputs_failed.len(), 2);
        assert!(caps.reason.is_some());
        assert_eq!(caps.reason.unwrap(), "some receipts failed to load");
    }

    #[test]
    fn test_capabilities_finds_check_ids_from_findings() {
        let receipts = vec![LoadedReceipt {
            path: "artifacts/sensor/report.json".into(),
            sensor_id: "sensor".to_string(),
            receipt: Ok(ReceiptEnvelope {
                schema: "sensor.report.v1".to_string(),
                tool: fixture_tool(),
                run: RunInfo {
                    started_at: Some(Utc::now()),
                    ended_at: Some(Utc::now()),
                    git_head_sha: None,
                },
                verdict: Verdict::default(),
                findings: vec![
                    Finding {
                        severity: Default::default(),
                        check_id: Some("rustc/W000".to_string()),
                        code: Some("unused_crate".to_string()),
                        message: Some("Unused crate".to_string()),
                        location: None,
                        fingerprint: None,
                        data: None,
                        ..Default::default()
                    },
                    Finding {
                        severity: Default::default(),
                        check_id: Some("clippy/DB01".to_string()),
                        code: Some("derives".to_string()),
                        message: Some("Derive issue".to_string()),
                        location: None,
                        fingerprint: None,
                        data: None,
                        ..Default::default()
                    },
                ],
                capabilities: None,
                data: None,
            }),
        }];

        let caps = build_report_capabilities(&receipts);
        assert!(caps.check_ids.contains(&"rustc/W000".to_string()));
        assert!(caps.check_ids.contains(&"clippy/DB01".to_string()));
    }

    #[test]
    fn test_plan_report_empty_plan_passes() {
        let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
        let report = build_plan_report(&plan, fixture_tool(), &[]);

        assert_eq!(report.verdict.status, ReportStatus::Pass);
        assert!(report.findings.is_empty());
        assert!(report.capabilities.is_some());
        let caps = report.capabilities.as_ref().unwrap();
        assert!(caps.inputs_failed.is_empty());
    }

    #[test]
    fn test_plan_report_with_ops_warns() {
        let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
        plan.ops.push(PlanOp {
            id: "op1".to_string(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".to_string(),
            },
            kind: OpKind::TomlSet {
                toml_path: vec!["workspace".to_string(), "members".to_string()],
                value: serde_json::json!(["crate1"]),
            },
            rationale: Rationale {
                fix_key: "unused-dependency".to_string(),
                description: Some("Remove unused dependency".to_string()),
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        });
        plan.summary = PlanSummary {
            ops_total: 1,
            ops_blocked: 0,
            files_touched: 1,
            patch_bytes: Some(100),
            safety_counts: Some(SafetyCounts {
                safe: 1,
                guarded: 0,
                unsafe_count: 0,
            }),
        };

        let report = build_plan_report(&plan, fixture_tool(), &[]);

        assert_eq!(report.verdict.status, ReportStatus::Warn);
        assert_eq!(report.verdict.counts.warn, 1);
        let data = report.data.as_ref().unwrap();
        let plan_data = &data["buildfix"]["plan"];
        assert_eq!(plan_data["ops_total"], 1);
        assert_eq!(plan_data["ops_applicable"], 1);
        assert_eq!(plan_data["fix_available"], true);
    }

    #[test]
    fn test_plan_report_with_blocked_ops() {
        let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
        plan.ops.push(PlanOp {
            id: "op1".to_string(),
            safety: SafetyClass::Unsafe,
            blocked: true,
            blocked_reason: Some("Missing parameters: version".to_string()),
            blocked_reason_token: Some("missing_params".to_string()),
            target: OpTarget {
                path: "Cargo.toml".to_string(),
            },
            kind: OpKind::TomlSet {
                toml_path: vec!["dependencies".to_string(), "foo".to_string()],
                value: serde_json::json!({"version": "PARAM"}),
            },
            rationale: Rationale {
                fix_key: "add-dependency".to_string(),
                description: Some("Add missing dependency".to_string()),
                findings: vec![],
            },
            params_required: vec!["version".to_string()],
            preview: None,
        });
        plan.summary = PlanSummary {
            ops_total: 1,
            ops_blocked: 1,
            files_touched: 1,
            patch_bytes: Some(50),
            safety_counts: Some(SafetyCounts {
                safe: 0,
                guarded: 0,
                unsafe_count: 1,
            }),
        };

        let report = build_plan_report(&plan, fixture_tool(), &[]);

        assert_eq!(report.verdict.status, ReportStatus::Warn);
        let data = report.data.as_ref().unwrap();
        let plan_data = &data["buildfix"]["plan"];
        assert_eq!(plan_data["ops_blocked"], 1);
        assert_eq!(plan_data["ops_applicable"], 0);
        assert_eq!(plan_data["fix_available"], false);
        assert!(plan_data["blocked_reason_tokens_top"].is_array());
    }

    #[test]
    fn test_plan_report_failed_inputs_overrides_pass() {
        let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
        plan.summary = PlanSummary::default();

        let report = build_plan_report(
            &plan,
            fixture_tool(),
            &[LoadedReceipt {
                path: "artifacts/broken/report.json".into(),
                sensor_id: "broken".to_string(),
                receipt: Err(ReceiptLoadError::Io {
                    message: "file missing".to_string(),
                }),
            }],
        );

        assert_eq!(report.verdict.status, ReportStatus::Warn);
        assert!(
            report
                .verdict
                .reasons
                .contains(&"partial_inputs".to_string())
        );
        assert!(!report.findings.is_empty());
    }

    #[test]
    fn test_plan_report_timestamp_format() {
        let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
        let report = build_plan_report(&plan, fixture_tool(), &[]);

        assert!(!report.run.started_at.is_empty());
        assert!(report.run.ended_at.is_some());
        let ended = report.run.ended_at.as_ref().unwrap();
        assert!(ended.contains('T'));
        assert!(ended.ends_with('Z') || ended.ends_with("+00:00"));
    }

    #[test]
    fn test_plan_report_with_safety_counts() {
        let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
        for i in 0..3 {
            plan.ops.push(PlanOp {
                id: format!("op{}", i),
                safety: if i == 0 {
                    SafetyClass::Safe
                } else {
                    SafetyClass::Guarded
                },
                blocked: false,
                blocked_reason: None,
                blocked_reason_token: None,
                target: OpTarget {
                    path: format!("Cargo{}.toml", i),
                },
                kind: OpKind::TomlSet {
                    toml_path: vec!["package".to_string(), "version".to_string()],
                    value: serde_json::json!("0.1.0"),
                },
                rationale: Rationale {
                    fix_key: "test".to_string(),
                    description: None,
                    findings: vec![],
                },
                params_required: vec![],
                preview: None,
            });
        }
        plan.summary = PlanSummary {
            ops_total: 3,
            ops_blocked: 0,
            files_touched: 3,
            patch_bytes: Some(300),
            safety_counts: Some(SafetyCounts {
                safe: 1,
                guarded: 2,
                unsafe_count: 0,
            }),
        };

        let report = build_plan_report(&plan, fixture_tool(), &[]);

        let data = report.data.as_ref().unwrap();
        let plan_data = &data["buildfix"]["plan"];
        let safety = &plan_data["safety_counts"];
        assert_eq!(safety["safe"], 1);
        assert_eq!(safety["guarded"], 2);
        assert_eq!(safety["unsafe"], 0);
    }

    #[test]
    fn test_apply_report_empty_applies_warns() {
        let apply = BuildfixApply::new(
            fixture_tool(),
            ApplyRepoInfo {
                root: ".".to_string(),
                head_sha_before: Some("abc123".to_string()),
                head_sha_after: Some("abc123".to_string()),
                dirty_before: Some(false),
                dirty_after: Some(false),
            },
            PlanRef {
                path: "plan.json".into(),
                sha256: None,
            },
        );

        let report = build_apply_report(&apply, fixture_tool());

        assert_eq!(report.verdict.status, ReportStatus::Warn);
        assert_eq!(report.verdict.counts.info, 0);
        assert_eq!(report.verdict.counts.warn, 0);
        assert_eq!(report.verdict.counts.error, 0);
    }

    #[test]
    fn test_apply_report_with_failures_fails() {
        let mut apply = BuildfixApply::new(
            fixture_tool(),
            ApplyRepoInfo {
                root: ".".to_string(),
                head_sha_before: Some("abc123".to_string()),
                head_sha_after: Some("def456".to_string()),
                dirty_before: Some(false),
                dirty_after: Some(true),
            },
            PlanRef {
                path: "plan.json".into(),
                sha256: Some("hash".to_string()),
            },
        );
        apply.summary.attempted = 5;
        apply.summary.applied = 3;
        apply.summary.blocked = 1;
        apply.summary.failed = 1;
        apply.summary.files_modified = 2;

        let report = build_apply_report(&apply, fixture_tool());

        assert_eq!(report.verdict.status, ReportStatus::Fail);
        assert_eq!(report.verdict.counts.error, 1);
    }

    #[test]
    fn test_apply_report_with_blocked_warns() {
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
        apply.summary.attempted = 3;
        apply.summary.applied = 1;
        apply.summary.blocked = 2;
        apply.summary.failed = 0;
        apply.summary.files_modified = 1;

        let report = build_apply_report(&apply, fixture_tool());

        assert_eq!(report.verdict.status, ReportStatus::Warn);
        assert_eq!(report.verdict.counts.warn, 2);
    }

    #[test]
    fn test_apply_report_passes_on_success() {
        let mut apply = BuildfixApply::new(
            fixture_tool(),
            ApplyRepoInfo {
                root: ".".to_string(),
                head_sha_before: Some("abc123".to_string()),
                head_sha_after: Some("def456".to_string()),
                dirty_before: Some(false),
                dirty_after: Some(true),
            },
            PlanRef {
                path: "plan.json".into(),
                sha256: Some("hash".to_string()),
            },
        );
        apply.summary.attempted = 2;
        apply.summary.applied = 2;
        apply.summary.blocked = 0;
        apply.summary.failed = 0;
        apply.summary.files_modified = 2;

        let report = build_apply_report(&apply, fixture_tool());

        assert_eq!(report.verdict.status, ReportStatus::Pass);
        assert_eq!(report.verdict.counts.info, 2);
    }

    #[test]
    fn test_apply_report_auto_commit_info() {
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
                path: "plan.json".to_string(),
                sha256: None,
            },
        );
        apply.summary.applied = 1;
        apply.auto_commit = Some(AutoCommitInfo {
            enabled: true,
            attempted: true,
            committed: true,
            commit_sha: Some("abc123def".to_string()),
            message: Some("chore: apply buildfix plan".to_string()),
            skip_reason: None,
        });

        let report = build_apply_report(&apply, fixture_tool());

        let data = report.data.as_ref().unwrap();
        let apply_data = &data["buildfix"]["apply"];
        assert_eq!(apply_data["auto_commit"]["enabled"], true);
        assert_eq!(apply_data["auto_commit"]["attempted"], true);
        assert_eq!(apply_data["auto_commit"]["committed"], true);
        assert_eq!(apply_data["auto_commit"]["commit_sha"], "abc123def");
    }

    #[test]
    fn test_apply_report_auto_commit_disabled() {
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
                path: "plan.json".to_string(),
                sha256: None,
            },
        );
        apply.summary.applied = 1;
        apply.auto_commit = Some(AutoCommitInfo {
            enabled: false,
            attempted: false,
            committed: false,
            commit_sha: None,
            message: None,
            skip_reason: Some("dirty working tree".to_string()),
        });

        let report = build_apply_report(&apply, fixture_tool());

        let data = report.data.as_ref().unwrap();
        let apply_data = &data["buildfix"]["apply"];
        assert_eq!(apply_data["auto_commit"]["enabled"], false);
        assert_eq!(
            apply_data["auto_commit"]["skip_reason"],
            "dirty working tree"
        );
    }

    #[test]
    fn test_apply_report_git_head_sha_tracking() {
        let apply = BuildfixApply::new(
            fixture_tool(),
            ApplyRepoInfo {
                root: ".".to_string(),
                head_sha_before: Some("before_sha".to_string()),
                head_sha_after: Some("after_sha".to_string()),
                dirty_before: Some(false),
                dirty_after: Some(false),
            },
            PlanRef {
                path: "plan.json".to_string(),
                sha256: None,
            },
        );

        let report = build_apply_report(&apply, fixture_tool());

        assert_eq!(report.run.git_head_sha, Some("after_sha".to_string()));
    }

    #[test]
    fn test_plan_report_git_head_sha_tracking() {
        let plan = BuildfixPlan::new(
            fixture_tool(),
            buildfix_types::plan::RepoInfo {
                root: ".".to_string(),
                head_sha: Some("test_sha".to_string()),
                dirty: Some(false),
            },
            PlanPolicy::default(),
        );

        let report = build_plan_report(&plan, fixture_tool(), &[]);

        assert_eq!(report.run.git_head_sha, Some("test_sha".to_string()));
    }

    #[test]
    fn test_plan_report_artifacts_present() {
        let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
        let report = build_plan_report(&plan, fixture_tool(), &[]);

        assert!(report.artifacts.is_some());
        let artifacts = report.artifacts.as_ref().unwrap();
        assert_eq!(artifacts.plan, Some("plan.json".to_string()));
        assert_eq!(artifacts.patch, Some("patch.diff".to_string()));
        assert_eq!(artifacts.comment, Some("comment.md".to_string()));
        assert!(artifacts.apply.is_none());
    }

    #[test]
    fn test_apply_report_artifacts_present() {
        let apply = BuildfixApply::new(
            fixture_tool(),
            ApplyRepoInfo {
                root: ".".to_string(),
                head_sha_before: None,
                head_sha_after: None,
                dirty_before: None,
                dirty_after: None,
            },
            PlanRef {
                path: "plan.json".to_string(),
                sha256: None,
            },
        );
        let report = build_apply_report(&apply, fixture_tool());

        assert!(report.artifacts.is_some());
        let artifacts = report.artifacts.as_ref().unwrap();
        assert_eq!(artifacts.plan, Some("plan.json".to_string()));
        assert_eq!(artifacts.apply, Some("apply.json".to_string()));
        assert_eq!(artifacts.patch, Some("patch.diff".to_string()));
        assert!(artifacts.comment.is_none());
    }

    #[test]
    fn test_plan_report_capabilities_partial_flag() {
        let receipts = vec![
            LoadedReceipt {
                path: "artifacts/ok/report.json".into(),
                sensor_id: "ok".to_string(),
                receipt: Ok(ReceiptEnvelope {
                    schema: "sensor.report.v1".to_string(),
                    tool: fixture_tool(),
                    run: RunInfo {
                        started_at: Some(Utc::now()),
                        ended_at: Some(Utc::now()),
                        git_head_sha: None,
                    },
                    verdict: Verdict::default(),
                    findings: vec![],
                    capabilities: None,
                    data: None,
                }),
            },
            LoadedReceipt {
                path: "artifacts/fail/report.json".into(),
                sensor_id: "fail".to_string(),
                receipt: Err(ReceiptLoadError::Io {
                    message: "boom".to_string(),
                }),
            },
        ];

        let caps = build_report_capabilities(&receipts);
        assert!(caps.partial);
        assert_eq!(caps.inputs_available.len(), 1);
        assert_eq!(caps.inputs_failed.len(), 1);
    }

    #[test]
    fn test_plan_report_inputs_sorted() {
        let receipts = vec![
            LoadedReceipt {
                path: "artifacts/z_report.json".into(),
                sensor_id: "z".to_string(),
                receipt: Ok(ReceiptEnvelope {
                    schema: "sensor.report.v1".to_string(),
                    tool: fixture_tool(),
                    run: RunInfo {
                        started_at: Some(Utc::now()),
                        ended_at: Some(Utc::now()),
                        git_head_sha: None,
                    },
                    verdict: Verdict::default(),
                    findings: vec![],
                    capabilities: None,
                    data: None,
                }),
            },
            LoadedReceipt {
                path: "artifacts/a_report.json".into(),
                sensor_id: "a".to_string(),
                receipt: Ok(ReceiptEnvelope {
                    schema: "sensor.report.v1".to_string(),
                    tool: fixture_tool(),
                    run: RunInfo {
                        started_at: Some(Utc::now()),
                        ended_at: Some(Utc::now()),
                        git_head_sha: None,
                    },
                    verdict: Verdict::default(),
                    findings: vec![],
                    capabilities: None,
                    data: None,
                }),
            },
        ];

        let caps = build_report_capabilities(&receipts);
        assert_eq!(
            caps.inputs_available,
            vec![
                "artifacts/a_report.json".to_string(),
                "artifacts/z_report.json".to_string(),
            ]
        );
    }

    #[test]
    fn test_plan_report_failed_inputs_sorted() {
        let receipts = vec![
            LoadedReceipt {
                path: "artifacts/z_fail.json".into(),
                sensor_id: "z".to_string(),
                receipt: Err(ReceiptLoadError::Io {
                    message: "error".to_string(),
                }),
            },
            LoadedReceipt {
                path: "artifacts/a_fail.json".into(),
                sensor_id: "a".to_string(),
                receipt: Err(ReceiptLoadError::Io {
                    message: "error".to_string(),
                }),
            },
        ];

        let caps = build_report_capabilities(&receipts);
        assert_eq!(caps.inputs_failed.len(), 2);
        assert_eq!(caps.inputs_failed[0].path, "artifacts/a_fail.json");
        assert_eq!(caps.inputs_failed[1].path, "artifacts/z_fail.json");
    }

    #[test]
    fn test_apply_report_data_structure() {
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
                path: "plan.json".to_string(),
                sha256: None,
            },
        );
        apply.summary.attempted = 10;
        apply.summary.applied = 7;
        apply.summary.blocked = 2;
        apply.summary.failed = 1;
        apply.summary.files_modified = 5;

        let report = build_apply_report(&apply, fixture_tool());

        let data = report.data.as_ref().unwrap();
        let apply_data = &data["buildfix"]["apply"];
        assert_eq!(apply_data["attempted"], 10);
        assert_eq!(apply_data["applied"], 7);
        assert_eq!(apply_data["blocked"], 2);
        assert_eq!(apply_data["failed"], 1);
        assert_eq!(apply_data["files_modified"], 5);
        assert_eq!(apply_data["apply_performed"], true);
    }

    #[test]
    fn test_apply_report_no_apply_performed() {
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
                path: "plan.json".to_string(),
                sha256: None,
            },
        );
        apply.summary.attempted = 0;
        apply.summary.applied = 0;

        let report = build_apply_report(&apply, fixture_tool());

        let data = report.data.as_ref().unwrap();
        let apply_data = &data["buildfix"]["apply"];
        assert_eq!(apply_data["apply_performed"], false);
    }

    #[test]
    fn test_plan_report_findings_fingerprint_format() {
        let receipts = vec![LoadedReceipt {
            path: "artifacts/test/report.json".into(),
            sensor_id: "test".to_string(),
            receipt: Err(ReceiptLoadError::Io {
                message: "file not found".to_string(),
            }),
        }];

        let report = build_plan_report(
            &BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default()),
            fixture_tool(),
            &receipts,
        );

        assert!(!report.findings.is_empty());
        let finding = &report.findings[0];
        assert_eq!(finding.code, "receipt_load_failed");
        assert!(finding.fingerprint.is_some());
        let fp = finding.fingerprint.as_ref().unwrap();
        assert!(fp.starts_with("inputs/receipt_load_failed/"));
    }
}
