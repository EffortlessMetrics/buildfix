use buildfix_types::apply::{ApplyRepoInfo, ApplyStatus, BuildfixApply, PlanRef};
use buildfix_types::ops::{OpKind, OpTarget};
use buildfix_types::plan::{BuildfixPlan, PlanPolicy, RepoInfo};
use buildfix_types::receipt::{
    Finding, Location, ReceiptCapabilities, ReceiptEnvelope, RunInfo, ToolInfo,
};
use buildfix_types::report::{
    BuildfixReport, ReportCapabilities, ReportCounts, ReportFinding, ReportLocation, ReportRunInfo,
    ReportSeverity, ReportStatus, ReportToolInfo, ReportVerdict,
};
use camino::Utf8PathBuf;

#[test]
fn apply_status_serializes_snake_case() {
    let applied = serde_json::to_value(ApplyStatus::Applied).expect("serialize");
    let blocked = serde_json::to_value(ApplyStatus::Blocked).expect("serialize");
    let failed = serde_json::to_value(ApplyStatus::Failed).expect("serialize");
    let skipped = serde_json::to_value(ApplyStatus::Skipped).expect("serialize");

    assert_eq!(applied, serde_json::json!("applied"));
    assert_eq!(blocked, serde_json::json!("blocked"));
    assert_eq!(failed, serde_json::json!("failed"));
    assert_eq!(skipped, serde_json::json!("skipped"));
}

#[test]
fn buildfix_apply_omits_empty_errors() {
    let apply = BuildfixApply::new(
        ToolInfo {
            name: "buildfix".to_string(),
            version: Some("1.0.0".to_string()),
            repo: None,
            commit: None,
        },
        ApplyRepoInfo {
            root: "/repo".to_string(),
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

    let value = serde_json::to_value(&apply).expect("serialize apply");
    assert!(value.get("errors").is_none());
}

#[test]
fn report_status_and_severity_serialize_snake_case() {
    let pass = serde_json::to_value(ReportStatus::Pass).expect("serialize");
    let warn = serde_json::to_value(ReportStatus::Warn).expect("serialize");
    let fail = serde_json::to_value(ReportStatus::Fail).expect("serialize");
    let skip = serde_json::to_value(ReportStatus::Skip).expect("serialize");

    assert_eq!(pass, serde_json::json!("pass"));
    assert_eq!(warn, serde_json::json!("warn"));
    assert_eq!(fail, serde_json::json!("fail"));
    assert_eq!(skip, serde_json::json!("skip"));

    let info = serde_json::to_value(ReportSeverity::Info).expect("serialize");
    let error = serde_json::to_value(ReportSeverity::Error).expect("serialize");
    assert_eq!(info, serde_json::json!("info"));
    assert_eq!(error, serde_json::json!("error"));
}

#[test]
fn report_omits_optional_sections_when_none() {
    let report = BuildfixReport {
        schema: buildfix_types::schema::BUILDFIX_REPORT_V1.to_string(),
        tool: ReportToolInfo {
            name: "buildfix".to_string(),
            version: "1.0.0".to_string(),
            commit: None,
        },
        run: ReportRunInfo {
            started_at: "2025-01-01T00:00:00Z".to_string(),
            ended_at: None,
            duration_ms: None,
            git_head_sha: None,
        },
        verdict: ReportVerdict {
            status: ReportStatus::Pass,
            counts: ReportCounts::default(),
            reasons: vec![],
        },
        findings: vec![],
        capabilities: None,
        artifacts: None,
        data: None,
    };

    let value = serde_json::to_value(&report).expect("serialize report");
    assert!(value.get("capabilities").is_none());
    assert!(value.get("artifacts").is_none());
    assert!(value.get("data").is_none());
}

#[test]
fn report_capabilities_serializes_empty_lists_as_empty_object() {
    let report = BuildfixReport {
        schema: buildfix_types::schema::BUILDFIX_REPORT_V1.to_string(),
        tool: ReportToolInfo {
            name: "buildfix".to_string(),
            version: "1.0.0".to_string(),
            commit: None,
        },
        run: ReportRunInfo {
            started_at: "2025-01-01T00:00:00Z".to_string(),
            ended_at: None,
            duration_ms: None,
            git_head_sha: None,
        },
        verdict: ReportVerdict {
            status: ReportStatus::Warn,
            counts: ReportCounts::default(),
            reasons: vec![],
        },
        findings: vec![ReportFinding {
            severity: ReportSeverity::Warn,
            check_id: None,
            code: "warn".to_string(),
            message: "message".to_string(),
            location: Some(ReportLocation {
                path: "Cargo.toml".to_string(),
                line: Some(1),
                col: None,
            }),
            fingerprint: None,
            data: None,
        }],
        capabilities: Some(ReportCapabilities::default()),
        artifacts: None,
        data: None,
    };

    let value = serde_json::to_value(&report).expect("serialize report");
    let caps = value.get("capabilities").expect("capabilities");
    assert!(caps.is_object());
}

#[test]
fn receipt_finding_defaults_and_location_serialization() {
    let raw = r#"{
        "schema": "sensor.report.v1",
        "tool": { "name": "builddiag", "version": "1.0.0" },
        "findings": [{}]
    }"#;

    let env: ReceiptEnvelope = serde_json::from_str(raw).expect("parse receipt");
    assert_eq!(env.findings.len(), 1);
    let finding = &env.findings[0];
    assert_eq!(finding.severity, buildfix_types::receipt::Severity::Info);
    assert!(finding.check_id.is_none());
    assert!(finding.code.is_none());
    assert!(finding.message.is_none());

    let finding = Finding {
        severity: buildfix_types::receipt::Severity::Warn,
        check_id: Some("id".to_string()),
        code: Some("code".to_string()),
        message: Some("msg".to_string()),
        location: Some(Location {
            path: Utf8PathBuf::from("src/lib.rs"),
            line: Some(1),
            column: None,
        }),
        fingerprint: None,
        data: None,
    };
    let value = serde_json::to_value(&finding).expect("serialize finding");
    assert_eq!(value["location"]["path"], serde_json::json!("src/lib.rs"));
}

#[test]
fn receipt_capabilities_serializes_partial_and_omits_empty_lists() {
    let env = ReceiptEnvelope {
        schema: buildfix_types::schema::SENSOR_REPORT_V1.to_string(),
        tool: ToolInfo {
            name: "builddiag".to_string(),
            version: Some("1.0.0".to_string()),
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Default::default(),
        findings: vec![],
        capabilities: Some(ReceiptCapabilities::default()),
        data: None,
    };

    let value = serde_json::to_value(&env).expect("serialize receipt");
    let caps = value.get("capabilities").expect("capabilities");
    assert_eq!(caps["partial"], serde_json::json!(false));
    assert!(caps.get("check_ids").is_none());
    assert!(caps.get("scopes").is_none());
    assert!(caps.get("reason").is_none());
}

#[test]
fn opkind_serializes_remove_and_transform() {
    let remove = OpKind::TomlRemove {
        toml_path: vec!["package".to_string(), "name".to_string()],
    };
    let remove_value = serde_json::to_value(&remove).expect("serialize remove");
    assert_eq!(remove_value["type"], "toml_remove");

    let transform = OpKind::TomlTransform {
        rule_id: "custom_rule".to_string(),
        args: None,
    };
    let transform_value = serde_json::to_value(&transform).expect("serialize transform");
    assert_eq!(transform_value["type"], "toml_transform");
    assert_eq!(transform_value["rule_id"], "custom_rule");
    assert!(transform_value.get("args").is_none());
}

#[test]
fn plan_op_with_transform_roundtrip() {
    let plan = BuildfixPlan::new(
        ToolInfo {
            name: "buildfix".to_string(),
            version: Some("1.0.0".to_string()),
            repo: None,
            commit: None,
        },
        RepoInfo {
            root: "/repo".to_string(),
            head_sha: None,
            dirty: None,
        },
        PlanPolicy::default(),
    );

    let op = buildfix_types::plan::PlanOp {
        id: "op1".to_string(),
        safety: buildfix_types::ops::SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        blocked_reason_token: None,
        target: OpTarget {
            path: "Cargo.toml".to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: "set_package_edition".to_string(),
            args: Some(serde_json::json!({"edition": "2021"})),
        },
        rationale: buildfix_types::plan::Rationale {
            fix_key: "test".to_string(),
            description: None,
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    };

    let mut plan = plan;
    plan.ops.push(op);
    plan.summary.ops_total = 1;

    let value = serde_json::to_value(&plan).expect("serialize plan");
    assert_eq!(value["ops"][0]["kind"]["rule_id"], "set_package_edition");
}
