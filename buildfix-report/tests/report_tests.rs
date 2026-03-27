//! Integration-style tests for buildfix-report crate.
//!
//! These tests complement the inline unit tests in src/lib.rs by covering
//! additional edge cases, large reports, and integration scenarios.

use buildfix_receipts::{LoadedReceipt, ReceiptLoadError};
use buildfix_report::{build_apply_report, build_plan_report, build_report_capabilities};
use buildfix_types::{
    apply::{ApplyRepoInfo, AutoCommitInfo, BuildfixApply, PlanRef},
    ops::{OpKind, OpTarget, SafetyClass},
    plan::{BuildfixPlan, PlanOp, PlanPolicy, PlanSummary, Rationale, RepoInfo, SafetyCounts},
    receipt::{
        Finding, ReceiptCapabilities, ReceiptEnvelope, RunInfo, Severity, ToolInfo, Verdict,
    },
    report::{ReportSeverity, ReportStatus},
};
use chrono::Utc;

/// Helper to create a fixture tool info.
fn fixture_tool() -> ToolInfo {
    ToolInfo {
        name: "buildfix".to_string(),
        version: Some("0.2.0".to_string()),
        repo: None,
        commit: None,
    }
}

/// Helper to create a default repo info for plans.
fn default_repo() -> RepoInfo {
    RepoInfo {
        root: ".".to_string(),
        head_sha: None,
        dirty: None,
    }
}

/// Helper to create a valid receipt envelope.
fn valid_receipt(path: &str, sensor_id: &str) -> LoadedReceipt {
    LoadedReceipt {
        path: path.into(),
        sensor_id: sensor_id.to_string(),
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
    }
}

/// Helper to create a failed receipt.
fn failed_receipt(path: &str, sensor_id: &str, message: &str) -> LoadedReceipt {
    LoadedReceipt {
        path: path.into(),
        sensor_id: sensor_id.to_string(),
        receipt: Err(ReceiptLoadError::Io {
            message: message.to_string(),
        }),
    }
}

// =============================================================================
// Report Generation Tests
// =============================================================================

#[test]
fn test_plan_report_schema_version() {
    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &[]);

    assert!(!report.schema.is_empty());
    assert!(report.schema.contains("report.v1"));
}

#[test]
fn test_apply_report_schema_version() {
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
            path: "plan.json".into(),
            sha256: None,
        },
    );
    let report = build_apply_report(&apply, fixture_tool());

    assert!(!report.schema.is_empty());
    assert!(report.schema.contains("report.v1"));
}

#[test]
fn test_plan_report_tool_info_preserved() {
    let tool = ToolInfo {
        name: "custom-tool".to_string(),
        version: Some("1.2.3".to_string()),
        repo: Some("https://github.com/example/tool".to_string()),
        commit: Some("abc123".to_string()),
    };
    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, tool.clone(), &[]);

    assert_eq!(report.tool.name, "custom-tool");
    assert_eq!(report.tool.version, "1.2.3");
    assert_eq!(report.tool.commit, Some("abc123".to_string()));
}

#[test]
fn test_apply_report_tool_info_no_version() {
    let tool = ToolInfo {
        name: "no-version-tool".to_string(),
        version: None,
        repo: None,
        commit: None,
    };
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
            path: "plan.json".into(),
            sha256: None,
        },
    );
    let report = build_apply_report(&apply, tool);

    assert_eq!(report.tool.name, "no-version-tool");
    assert_eq!(report.tool.version, "unknown");
    assert_eq!(report.tool.commit, None);
}

// =============================================================================
// Summary Statistics Tests
// =============================================================================

#[test]
fn test_plan_report_summary_statistics() {
    let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    plan.summary = PlanSummary {
        ops_total: 10,
        ops_blocked: 3,
        files_touched: 5,
        patch_bytes: Some(1500),
        safety_counts: Some(SafetyCounts {
            safe: 5,
            guarded: 2,
            unsafe_count: 3,
        }),
    };

    let report = build_plan_report(&plan, fixture_tool(), &[]);
    let data = report.data.as_ref().unwrap();
    let plan_data = &data["buildfix"]["plan"];

    assert_eq!(plan_data["ops_total"], 10);
    assert_eq!(plan_data["ops_blocked"], 3);
    assert_eq!(plan_data["ops_applicable"], 7); // 10 - 3
    assert_eq!(plan_data["files_touched"], 5);
    assert_eq!(plan_data["patch_bytes"], 1500);
}

#[test]
fn test_plan_report_safety_counts_breakdown() {
    let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    plan.summary = PlanSummary {
        ops_total: 100,
        ops_blocked: 0,
        files_touched: 10,
        patch_bytes: Some(5000),
        safety_counts: Some(SafetyCounts {
            safe: 60,
            guarded: 30,
            unsafe_count: 10,
        }),
    };

    let report = build_plan_report(&plan, fixture_tool(), &[]);
    let data = report.data.as_ref().unwrap();
    let safety = &data["buildfix"]["plan"]["safety_counts"];

    assert_eq!(safety["safe"], 60);
    assert_eq!(safety["guarded"], 30);
    assert_eq!(safety["unsafe"], 10);
}

#[test]
fn test_plan_report_no_safety_counts_when_none() {
    let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    plan.summary = PlanSummary {
        ops_total: 5,
        ops_blocked: 0,
        files_touched: 2,
        patch_bytes: None,
        safety_counts: None,
    };

    let report = build_plan_report(&plan, fixture_tool(), &[]);
    let data = report.data.as_ref().unwrap();
    let plan_data = &data["buildfix"]["plan"];

    // safety_counts should not be present when None
    assert!(plan_data.get("safety_counts").is_none());
}

#[test]
fn test_apply_report_summary_statistics() {
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
    apply.summary.attempted = 20;
    apply.summary.applied = 15;
    apply.summary.blocked = 3;
    apply.summary.failed = 2;
    apply.summary.files_modified = 8;

    let report = build_apply_report(&apply, fixture_tool());
    let data = report.data.as_ref().unwrap();
    let apply_data = &data["buildfix"]["apply"];

    assert_eq!(apply_data["attempted"], 20);
    assert_eq!(apply_data["applied"], 15);
    assert_eq!(apply_data["blocked"], 3);
    assert_eq!(apply_data["failed"], 2);
    assert_eq!(apply_data["files_modified"], 8);
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn test_capabilities_mixed_success_and_failure() {
    let receipts = vec![
        valid_receipt("artifacts/sensor1/report.json", "sensor1"),
        failed_receipt("artifacts/sensor2/report.json", "sensor2", "file not found"),
        valid_receipt("artifacts/sensor3/report.json", "sensor3"),
        failed_receipt(
            "artifacts/sensor4/report.json",
            "sensor4",
            "permission denied",
        ),
    ];

    let caps = build_report_capabilities(&receipts);

    assert!(caps.partial);
    assert_eq!(caps.inputs_available.len(), 2);
    assert_eq!(caps.inputs_failed.len(), 2);
    assert!(caps.reason.is_some());
}

#[test]
fn test_plan_report_json_error_in_receipt() {
    let receipts = vec![LoadedReceipt {
        path: "artifacts/bad/report.json".into(),
        sensor_id: "bad".to_string(),
        receipt: Err(ReceiptLoadError::Json {
            message: "invalid JSON at position 42".to_string(),
        }),
    }];

    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &receipts);

    assert_eq!(report.verdict.status, ReportStatus::Warn);
    assert!(!report.findings.is_empty());
    assert!(
        report.findings[0]
            .message
            .contains("invalid JSON at position 42")
    );
}

#[test]
fn test_plan_report_schema_validation_error_in_receipt() {
    // Simulate a JSON parse error that could be from schema validation
    let receipts = vec![LoadedReceipt {
        path: "artifacts/bad/report.json".into(),
        sensor_id: "bad".to_string(),
        receipt: Err(ReceiptLoadError::Json {
            message: "schema validation failed: missing required field".to_string(),
        }),
    }];

    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &receipts);

    assert_eq!(report.verdict.status, ReportStatus::Warn);
    assert!(!report.findings.is_empty());
    assert!(
        report.findings[0]
            .message
            .contains("schema validation failed: missing required field")
    );
}

#[test]
fn test_capabilities_io_vs_json_errors() {
    let receipts = vec![
        LoadedReceipt {
            path: "artifacts/io_error/report.json".into(),
            sensor_id: "io".to_string(),
            receipt: Err(ReceiptLoadError::Io {
                message: "file not found".to_string(),
            }),
        },
        LoadedReceipt {
            path: "artifacts/json_error/report.json".into(),
            sensor_id: "json".to_string(),
            receipt: Err(ReceiptLoadError::Json {
                message: "parse error".to_string(),
            }),
        },
    ];

    let caps = build_report_capabilities(&receipts);

    assert_eq!(caps.inputs_failed.len(), 2);
    // Both errors should be recorded with their respective messages
    let reasons: Vec<&str> = caps
        .inputs_failed
        .iter()
        .map(|f| f.reason.as_str())
        .collect();
    assert!(reasons.iter().any(|r| r.contains("file not found")));
    assert!(reasons.iter().any(|r| r.contains("parse error")));
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn test_empty_receipts_list() {
    let caps = build_report_capabilities(&[]);

    assert!(!caps.partial);
    assert!(caps.check_ids.is_empty());
    assert!(caps.scopes.is_empty());
    assert!(caps.inputs_available.is_empty());
    assert!(caps.inputs_failed.is_empty());
    assert!(caps.reason.is_none());
}

#[test]
fn test_plan_report_empty_plan() {
    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &[]);

    // Empty plan with no failed inputs should pass
    assert_eq!(report.verdict.status, ReportStatus::Pass);
    assert_eq!(report.verdict.counts.warn, 0);
    assert_eq!(report.verdict.counts.error, 0);
    assert!(report.findings.is_empty());
}

#[test]
fn test_apply_report_empty_apply() {
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
            path: "plan.json".into(),
            sha256: None,
        },
    );
    let report = build_apply_report(&apply, fixture_tool());

    // Empty apply (no operations) should warn
    assert_eq!(report.verdict.status, ReportStatus::Warn);
    assert_eq!(report.verdict.counts.info, 0);
    assert_eq!(report.verdict.counts.warn, 0);
    assert_eq!(report.verdict.counts.error, 0);
}

#[test]
fn test_plan_report_large_number_of_ops() {
    let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());

    // Add 1000 operations
    for i in 0..1000 {
        plan.ops.push(PlanOp {
            id: format!("op-{}", i),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: format!("crate{}/Cargo.toml", i % 10),
            },
            kind: OpKind::TomlSet {
                toml_path: vec!["package".to_string(), "version".to_string()],
                value: serde_json::json!("0.1.0"),
            },
            rationale: Rationale {
                fix_key: "test".to_string(),
                description: Some(format!("Test operation {}", i)),
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        });
    }
    plan.summary = PlanSummary {
        ops_total: 1000,
        ops_blocked: 0,
        files_touched: 10,
        patch_bytes: Some(50000),
        safety_counts: Some(SafetyCounts {
            safe: 1000,
            guarded: 0,
            unsafe_count: 0,
        }),
    };

    let report = build_plan_report(&plan, fixture_tool(), &[]);

    assert_eq!(report.verdict.status, ReportStatus::Warn);
    assert_eq!(report.verdict.counts.warn, 1000);

    let data = report.data.as_ref().unwrap();
    let plan_data = &data["buildfix"]["plan"];
    assert_eq!(plan_data["ops_total"], 1000);
    assert_eq!(plan_data["files_touched"], 10);
}

#[test]
fn test_plan_report_many_blocked_reason_tokens() {
    let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());

    // Create ops with many different blocked reason tokens
    let tokens = [
        "missing_param_a",
        "missing_param_b",
        "policy_denied",
        "precondition_failed",
        "unsafe_operation",
        "requires_confirmation",
        "guarded_needs_flag",
    ];

    for (i, token) in tokens.iter().enumerate() {
        plan.ops.push(PlanOp {
            id: format!("op-{}", i),
            safety: SafetyClass::Unsafe,
            blocked: true,
            blocked_reason: Some(format!("Blocked: {}", token)),
            blocked_reason_token: Some(token.to_string()),
            target: OpTarget {
                path: "Cargo.toml".to_string(),
            },
            kind: OpKind::TomlSet {
                toml_path: vec!["test".to_string()],
                value: serde_json::json!(true),
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
        ops_total: 7,
        ops_blocked: 7,
        files_touched: 1,
        patch_bytes: Some(100),
        safety_counts: Some(SafetyCounts {
            safe: 0,
            guarded: 0,
            unsafe_count: 7,
        }),
    };

    let report = build_plan_report(&plan, fixture_tool(), &[]);
    let data = report.data.as_ref().unwrap();
    let plan_data = &data["buildfix"]["plan"];

    // Should show top 5 blocked reason tokens
    let blocked_tokens = plan_data.get("blocked_reason_tokens_top");
    assert!(blocked_tokens.is_some());
    let tokens_array = blocked_tokens.unwrap().as_array().unwrap();
    assert!(tokens_array.len() <= 5);
}

#[test]
fn test_receipt_with_empty_check_id() {
    let receipts = vec![LoadedReceipt {
        path: "artifacts/empty_check/report.json".into(),
        sensor_id: "empty_check".to_string(),
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
                severity: Severity::Warn,
                check_id: Some("".to_string()), // Empty check ID
                code: Some("test".to_string()),
                message: Some("Test finding".to_string()),
                location: None,
                fingerprint: None,
                data: None,
                confidence: None,
                provenance: None,
                context: None,
            }],
            capabilities: None,
            data: None,
        }),
    }];

    let caps = build_report_capabilities(&receipts);

    // Empty check IDs should not be included
    assert!(caps.check_ids.is_empty());
}

#[test]
fn test_receipt_with_none_check_id() {
    let receipts = vec![LoadedReceipt {
        path: "artifacts/none_check/report.json".into(),
        sensor_id: "none_check".to_string(),
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
                severity: Severity::Warn,
                check_id: None, // None check ID
                code: Some("test".to_string()),
                message: Some("Test finding".to_string()),
                location: None,
                fingerprint: None,
                data: None,
                confidence: None,
                provenance: None,
                context: None,
            }],
            capabilities: None,
            data: None,
        }),
    }];

    let caps = build_report_capabilities(&receipts);

    // None check IDs should not cause issues
    assert!(caps.check_ids.is_empty());
}

#[test]
fn test_plan_report_with_patch_bytes_none() {
    let mut plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    plan.summary = PlanSummary {
        ops_total: 1,
        ops_blocked: 0,
        files_touched: 1,
        patch_bytes: None,
        safety_counts: None,
    };

    let report = build_plan_report(&plan, fixture_tool(), &[]);
    let data = report.data.as_ref().unwrap();
    let plan_data = &data["buildfix"]["plan"];

    // patch_bytes should be null when None
    assert!(plan_data.get("patch_bytes").unwrap().is_null());
}

// =============================================================================
// Findings Tests
// =============================================================================

#[test]
fn test_plan_report_finding_severity_is_warn() {
    let receipts = vec![failed_receipt(
        "artifacts/fail/report.json",
        "fail",
        "error message",
    )];

    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &receipts);

    assert!(!report.findings.is_empty());
    assert_eq!(report.findings[0].severity, ReportSeverity::Warn);
}

#[test]
fn test_plan_report_finding_check_id_is_inputs() {
    let receipts = vec![failed_receipt(
        "artifacts/fail/report.json",
        "fail",
        "error",
    )];

    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &receipts);

    assert_eq!(report.findings[0].check_id, Some("inputs".to_string()));
}

#[test]
fn test_plan_report_multiple_failed_inputs_generate_multiple_findings() {
    let receipts = vec![
        failed_receipt("artifacts/fail1/report.json", "fail1", "error1"),
        failed_receipt("artifacts/fail2/report.json", "fail2", "error2"),
        failed_receipt("artifacts/fail3/report.json", "fail3", "error3"),
    ];

    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &receipts);

    assert_eq!(report.findings.len(), 3);
}

// =============================================================================
// Verdict and Reasons Tests
// =============================================================================

#[test]
fn test_plan_report_reasons_includes_partial_inputs() {
    let receipts = vec![failed_receipt(
        "artifacts/fail/report.json",
        "fail",
        "error",
    )];

    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &receipts);

    assert!(
        report
            .verdict
            .reasons
            .contains(&"partial_inputs".to_string())
    );
}

#[test]
fn test_apply_report_verdict_reasons_empty_on_success() {
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
    apply.summary.applied = 5;

    let report = build_apply_report(&apply, fixture_tool());

    assert!(report.verdict.reasons.is_empty());
}

// =============================================================================
// Capabilities Scopes Tests
// =============================================================================

#[test]
fn test_capabilities_aggregates_scopes() {
    let receipts = vec![LoadedReceipt {
        path: "artifacts/scopes/report.json".into(),
        sensor_id: "scopes".to_string(),
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
            capabilities: Some(ReceiptCapabilities {
                check_ids: vec!["check1".to_string()],
                scopes: vec![
                    "workspace".to_string(),
                    "crate".to_string(),
                    "file".to_string(),
                ],
                partial: false,
                reason: None,
            }),
            data: None,
        }),
    }];

    let caps = build_report_capabilities(&receipts);

    assert_eq!(caps.scopes.len(), 3);
    assert!(caps.scopes.contains(&"workspace".to_string()));
    assert!(caps.scopes.contains(&"crate".to_string()));
    assert!(caps.scopes.contains(&"file".to_string()));
}

#[test]
fn test_capabilities_deduplicates_scopes() {
    let receipts = vec![
        LoadedReceipt {
            path: "artifacts/a/report.json".into(),
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
                capabilities: Some(ReceiptCapabilities {
                    check_ids: vec![],
                    scopes: vec!["workspace".to_string(), "crate".to_string()],
                    partial: false,
                    reason: None,
                }),
                data: None,
            }),
        },
        LoadedReceipt {
            path: "artifacts/b/report.json".into(),
            sensor_id: "b".to_string(),
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
                capabilities: Some(ReceiptCapabilities {
                    check_ids: vec![],
                    scopes: vec!["crate".to_string(), "file".to_string()],
                    partial: false,
                    reason: None,
                }),
                data: None,
            }),
        },
    ];

    let caps = build_report_capabilities(&receipts);

    // Scopes should be deduplicated and sorted
    assert_eq!(caps.scopes, vec!["crate", "file", "workspace"]);
}

// =============================================================================
// Auto Commit Edge Cases Tests
// =============================================================================

#[test]
fn test_apply_report_auto_commit_partial_info() {
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
    apply.summary.applied = 1;
    apply.auto_commit = Some(AutoCommitInfo {
        enabled: true,
        attempted: true,
        committed: false, // Attempted but not committed
        commit_sha: None,
        message: Some("chore: apply plan".to_string()),
        skip_reason: Some("pre-commit hook failed".to_string()),
    });

    let report = build_apply_report(&apply, fixture_tool());
    let data = report.data.as_ref().unwrap();
    let auto_commit = &data["buildfix"]["apply"]["auto_commit"];

    assert_eq!(auto_commit["enabled"], true);
    assert_eq!(auto_commit["attempted"], true);
    assert_eq!(auto_commit["committed"], false);
    assert_eq!(auto_commit["commit_sha"], serde_json::Value::Null);
    assert_eq!(auto_commit["skip_reason"], "pre-commit hook failed");
}

#[test]
fn test_apply_report_no_auto_commit() {
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
            path: "plan.json".into(),
            sha256: None,
        },
    );

    let report = build_apply_report(&apply, fixture_tool());
    let data = report.data.as_ref().unwrap();
    let apply_data = &data["buildfix"]["apply"];

    // auto_commit should not be present when None
    assert!(apply_data.get("auto_commit").is_none());
}

// =============================================================================
// Duration and Timing Tests
// =============================================================================

#[test]
fn test_plan_report_duration_is_zero() {
    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &[]);

    // Duration should be 0 for synchronous report generation
    assert_eq!(report.run.duration_ms, Some(0));
}

#[test]
fn test_apply_report_duration_is_zero() {
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
            path: "plan.json".into(),
            sha256: None,
        },
    );
    let report = build_apply_report(&apply, fixture_tool());

    assert_eq!(report.run.duration_ms, Some(0));
}

#[test]
fn test_plan_report_timestamps_are_valid_rfc3339() {
    let plan = BuildfixPlan::new(fixture_tool(), default_repo(), PlanPolicy::default());
    let report = build_plan_report(&plan, fixture_tool(), &[]);

    // Both timestamps should be valid RFC3339 format
    assert!(!report.run.started_at.is_empty());
    assert!(report.run.ended_at.is_some());

    let started = report.run.started_at;
    let ended = report.run.ended_at.unwrap();

    // Should contain 'T' separator and timezone info
    assert!(started.contains('T'));
    assert!(ended.contains('T'));

    // Should parse as valid RFC3339 timestamps
    chrono::DateTime::parse_from_rfc3339(&started).expect("started_at should be valid RFC3339");
    chrono::DateTime::parse_from_rfc3339(&ended).expect("ended_at should be valid RFC3339");
}

// =============================================================================
// Status Priority Tests
// =============================================================================

#[test]
fn test_apply_report_fail_takes_priority_over_warn() {
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
    apply.summary.failed = 1;
    apply.summary.blocked = 5; // Also has blocked

    let report = build_apply_report(&apply, fixture_tool());

    // Fail takes priority
    assert_eq!(report.verdict.status, ReportStatus::Fail);
}

#[test]
fn test_apply_report_warn_when_blocked_and_no_fail() {
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
    apply.summary.failed = 0;
    apply.summary.blocked = 5;
    apply.summary.applied = 10;

    let report = build_apply_report(&apply, fixture_tool());

    // Warn when blocked but no failures
    assert_eq!(report.verdict.status, ReportStatus::Warn);
}

#[test]
fn test_apply_report_pass_only_when_applied_and_no_issues() {
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
    apply.summary.failed = 0;
    apply.summary.blocked = 0;
    apply.summary.applied = 5;

    let report = build_apply_report(&apply, fixture_tool());

    assert_eq!(report.verdict.status, ReportStatus::Pass);
}

// =============================================================================
// Integration Tests
// =============================================================================

#[test]
fn test_full_plan_report_workflow() {
    // Simulate a realistic workflow with multiple receipts and a plan
    let receipts = vec![
        valid_receipt("artifacts/clippy/report.json", "clippy"),
        valid_receipt("artifacts/machete/report.json", "machete"),
    ];

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
            value: serde_json::json!(["crate1", "crate2"]),
        },
        rationale: Rationale {
            fix_key: "workspace-members".to_string(),
            description: Some("Update workspace members".to_string()),
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    });
    plan.summary = PlanSummary {
        ops_total: 1,
        ops_blocked: 0,
        files_touched: 1,
        patch_bytes: Some(50),
        safety_counts: Some(SafetyCounts {
            safe: 1,
            guarded: 0,
            unsafe_count: 0,
        }),
    };

    let report = build_plan_report(&plan, fixture_tool(), &receipts);

    // Verify complete report structure
    assert!(!report.schema.is_empty());
    assert_eq!(report.tool.name, "buildfix");
    assert!(report.run.started_at.contains('T'));
    assert_eq!(report.verdict.status, ReportStatus::Warn);
    assert!(report.capabilities.is_some());
    assert!(report.artifacts.is_some());
    assert!(report.data.is_some());
}

#[test]
fn test_full_apply_report_workflow() {
    let mut apply = BuildfixApply::new(
        fixture_tool(),
        ApplyRepoInfo {
            root: "/workspace/myproject".to_string(),
            head_sha_before: Some("abc123".to_string()),
            head_sha_after: Some("def456".to_string()),
            dirty_before: Some(false),
            dirty_after: Some(false),
        },
        PlanRef {
            path: "plan.json".into(),
            sha256: Some("sha256:abc".to_string()),
        },
    );
    apply.summary.attempted = 5;
    apply.summary.applied = 5;
    apply.summary.blocked = 0;
    apply.summary.failed = 0;
    apply.summary.files_modified = 3;
    apply.auto_commit = Some(AutoCommitInfo {
        enabled: true,
        attempted: true,
        committed: true,
        commit_sha: Some("def456".to_string()),
        message: Some("chore: apply buildfix plan".to_string()),
        skip_reason: None,
    });

    let report = build_apply_report(&apply, fixture_tool());

    // Verify complete report structure
    assert!(!report.schema.is_empty());
    assert_eq!(report.tool.name, "buildfix");
    assert_eq!(report.run.git_head_sha, Some("def456".to_string()));
    assert_eq!(report.verdict.status, ReportStatus::Pass);
    assert_eq!(report.verdict.counts.info, 5);
    assert!(report.artifacts.is_some());
    let artifacts = report.artifacts.as_ref().unwrap();
    assert_eq!(artifacts.apply, Some("apply.json".to_string()));
}
