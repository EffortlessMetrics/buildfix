use buildfix_types::apply::{ApplyRepoInfo, BuildfixApply, PlanRef};
use buildfix_types::ops::{OpKind, SafetyClass};
use buildfix_types::plan::{BuildfixPlan, PlanPolicy, RepoInfo};
use buildfix_types::receipt::{ReceiptEnvelope, ToolInfo, VerdictStatus};

#[test]
fn buildfix_plan_new_sets_schema_and_defaults() {
    let tool = ToolInfo {
        name: "buildfix".to_string(),
        version: Some("1.2.3".to_string()),
        repo: None,
        commit: None,
    };
    let repo = RepoInfo {
        root: "/repo".to_string(),
        head_sha: None,
        dirty: None,
    };
    let policy = PlanPolicy {
        allow: vec!["cargo.*".to_string()],
        deny: vec!["cargo.deny".to_string()],
        allow_guarded: true,
        allow_unsafe: false,
        allow_dirty: false,
        max_ops: Some(10),
        max_files: None,
        max_patch_bytes: None,
    };

    let plan = BuildfixPlan::new(tool.clone(), repo.clone(), policy.clone());

    assert_eq!(plan.schema, buildfix_types::schema::BUILDFIX_PLAN_V1);
    assert_eq!(plan.tool.name, tool.name);
    assert_eq!(plan.tool.version, tool.version);
    assert_eq!(plan.repo.root, repo.root);
    assert!(plan.inputs.is_empty());
    assert!(plan.ops.is_empty());
    assert!(plan.preconditions.files.is_empty());
    assert_eq!(plan.policy.allow, policy.allow);
    assert_eq!(plan.policy.deny, policy.deny);
    assert!(plan.policy.allow_guarded);
    assert_eq!(plan.summary.ops_total, 0);
    assert!(plan.summary.safety_counts.is_none());
}

#[test]
fn buildfix_apply_new_sets_schema_and_defaults() {
    let tool = ToolInfo {
        name: "buildfix".to_string(),
        version: Some("1.2.3".to_string()),
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

    let apply = BuildfixApply::new(tool, repo, plan_ref);

    assert_eq!(apply.schema, buildfix_types::schema::BUILDFIX_APPLY_V1);
    assert!(apply.results.is_empty());
    assert!(apply.errors.is_empty());
    assert_eq!(apply.summary.attempted, 0);
    assert_eq!(apply.summary.applied, 0);
    assert_eq!(apply.summary.blocked, 0);
    assert_eq!(apply.summary.failed, 0);
    assert_eq!(apply.summary.files_modified, 0);
}

#[test]
fn safety_class_helpers_match_variant() {
    assert!(SafetyClass::Safe.is_safe());
    assert!(!SafetyClass::Safe.is_guarded());
    assert!(!SafetyClass::Safe.is_unsafe());

    assert!(SafetyClass::Guarded.is_guarded());
    assert!(!SafetyClass::Guarded.is_safe());

    assert!(SafetyClass::Unsafe.is_unsafe());
    assert!(!SafetyClass::Unsafe.is_safe());
}

#[test]
fn op_kind_serializes_with_type_tag() {
    let op = OpKind::TomlSet {
        toml_path: vec!["workspace".to_string(), "resolver".to_string()],
        value: serde_json::json!("2"),
    };

    let value = serde_json::to_value(&op).expect("serialize op");
    assert_eq!(value["type"], "toml_set");
    assert_eq!(
        value["toml_path"],
        serde_json::json!(["workspace", "resolver"])
    );
    assert_eq!(value["value"], serde_json::json!("2"));
}

#[test]
fn receipt_envelope_defaults_missing_fields() {
    let raw = r#"{
        "schema": "sensor.report.v1",
        "tool": { "name": "builddiag" }
    }"#;

    let env: ReceiptEnvelope = serde_json::from_str(raw).expect("parse receipt");
    assert_eq!(env.schema, "sensor.report.v1");
    assert_eq!(env.tool.name, "builddiag");
    assert_eq!(env.verdict.status, VerdictStatus::Unknown);
    assert!(env.run.started_at.is_none());
    assert!(env.findings.is_empty());
    assert!(env.capabilities.is_none());
}
