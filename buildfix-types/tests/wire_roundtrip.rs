use buildfix_types::apply::{ApplyPreconditions, ApplyRepoInfo, ApplySummary, BuildfixApply, PlanRef};
use buildfix_types::plan::{BuildfixPlan, PlanPolicy, PlanPreconditions, PlanSummary, RepoInfo};
use buildfix_types::receipt::ToolInfo;
use buildfix_types::report::{
    BuildfixReport, ReportCounts, ReportRunInfo, ReportSeverity, ReportStatus, ReportToolInfo,
    ReportVerdict,
};
use buildfix_types::wire::{ApplyV1, PlanV1, ReportV1, WireError};

#[test]
fn plan_wire_requires_tool_version() {
    let tool = ToolInfo {
        name: "buildfix".to_string(),
        version: None,
        repo: None,
        commit: None,
    };
    let repo = RepoInfo {
        root: "/repo".to_string(),
        head_sha: None,
        dirty: None,
    };
    let policy = PlanPolicy::default();
    let plan = BuildfixPlan::new(tool, repo, policy);

    let err = PlanV1::try_from(&plan).expect_err("missing version should error");
    assert!(matches!(err, WireError::MissingToolVersion { context: "plan" }));
}

#[test]
fn plan_wire_roundtrip_preserves_tool_version() {
    let tool = ToolInfo {
        name: "buildfix".to_string(),
        version: Some("1.0.0".to_string()),
        repo: None,
        commit: Some("abc".to_string()),
    };
    let repo = RepoInfo {
        root: "/repo".to_string(),
        head_sha: None,
        dirty: None,
    };
    let policy = PlanPolicy::default();
    let plan = BuildfixPlan {
        schema: buildfix_types::schema::BUILDFIX_PLAN_V1.to_string(),
        tool,
        repo,
        inputs: vec![],
        policy,
        preconditions: PlanPreconditions::default(),
        ops: vec![],
        summary: PlanSummary::default(),
    };

    let wire = PlanV1::try_from(&plan).expect("wire conversion");
    assert_eq!(wire.tool.version, "1.0.0");

    let roundtrip: BuildfixPlan = wire.into();
    assert_eq!(roundtrip.tool.version.as_deref(), Some("1.0.0"));
    assert_eq!(roundtrip.tool.commit.as_deref(), Some("abc"));
}

#[test]
fn apply_wire_requires_tool_version() {
    let tool = ToolInfo {
        name: "buildfix".to_string(),
        version: None,
        repo: None,
        commit: None,
    };
    let repo = ApplyRepoInfo {
        root: "/repo".to_string(),
        head_sha_before: None,
        head_sha_after: None,
        dirty_before: None,
        dirty_after: None,
    };
    let plan_ref = PlanRef {
        path: "artifacts/buildfix/plan.json".to_string(),
        sha256: None,
    };
    let apply = BuildfixApply {
        schema: buildfix_types::schema::BUILDFIX_APPLY_V1.to_string(),
        tool,
        repo,
        plan_ref,
        preconditions: ApplyPreconditions::default(),
        results: vec![],
        summary: ApplySummary::default(),
        errors: vec![],
    };

    let err = ApplyV1::try_from(&apply).expect_err("missing version should error");
    assert!(matches!(err, WireError::MissingToolVersion { context: "apply" }));
}

#[test]
fn apply_wire_roundtrip_preserves_tool_version() {
    let tool = ToolInfo {
        name: "buildfix".to_string(),
        version: Some("1.0.0".to_string()),
        repo: None,
        commit: Some("def".to_string()),
    };
    let repo = ApplyRepoInfo {
        root: "/repo".to_string(),
        head_sha_before: None,
        head_sha_after: None,
        dirty_before: None,
        dirty_after: None,
    };
    let plan_ref = PlanRef {
        path: "artifacts/buildfix/plan.json".to_string(),
        sha256: Some("deadbeef".to_string()),
    };
    let apply = BuildfixApply {
        schema: buildfix_types::schema::BUILDFIX_APPLY_V1.to_string(),
        tool,
        repo,
        plan_ref,
        preconditions: ApplyPreconditions::default(),
        results: vec![],
        summary: ApplySummary::default(),
        errors: vec![],
    };

    let wire = ApplyV1::try_from(&apply).expect("wire conversion");
    assert_eq!(wire.tool.version, "1.0.0");

    let roundtrip: BuildfixApply = wire.into();
    assert_eq!(roundtrip.tool.version.as_deref(), Some("1.0.0"));
    assert_eq!(roundtrip.tool.commit.as_deref(), Some("def"));
}

#[test]
fn report_wire_from_buildfix_report() {
    let report = BuildfixReport {
        schema: buildfix_types::schema::BUILDFIX_REPORT_V1.to_string(),
        tool: ReportToolInfo {
            name: "buildfix".to_string(),
            version: "1.0.0".to_string(),
            commit: Some("abc".to_string()),
        },
        run: ReportRunInfo {
            started_at: "2025-01-01T00:00:00Z".to_string(),
            ended_at: None,
            duration_ms: None,
        },
        verdict: ReportVerdict {
            status: ReportStatus::Pass,
            counts: ReportCounts::default(),
            reasons: vec![],
        },
        findings: vec![buildfix_types::report::ReportFinding {
            severity: ReportSeverity::Info,
            check_id: None,
            code: "ok".to_string(),
            message: "all good".to_string(),
            location: None,
            fingerprint: None,
            data: None,
        }],
        capabilities: None,
        artifacts: None,
        data: None,
    };

    let wire = ReportV1::from(&report);
    assert_eq!(wire.schema, buildfix_types::schema::BUILDFIX_REPORT_V1);
    assert_eq!(wire.tool.version, "1.0.0");
    assert_eq!(wire.tool.commit.as_deref(), Some("abc"));
    assert_eq!(wire.findings.len(), 1);
}
