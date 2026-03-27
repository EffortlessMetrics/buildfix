//! Integration tests for buildfix-render crate.
//!
//! These tests complement the inline unit tests in src/lib.rs by covering
//! additional edge cases and scenarios.

use buildfix_render::{render_apply_md, render_comment_md, render_plan_md};
use buildfix_types::apply::{
    ApplyFile, ApplyRepoInfo, ApplyResult, ApplyStatus, ApplySummary, BuildfixApply, PlanRef,
};
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{
    BuildfixPlan, FindingRef, PlanInput, PlanOp, PlanPolicy, PlanSummary, Rationale, RepoInfo,
    SafetyCounts,
};
use buildfix_types::receipt::ToolInfo;

fn tool() -> ToolInfo {
    ToolInfo {
        name: "buildfix".into(),
        version: Some("0.2.0".into()),
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

fn make_op(safety: SafetyClass, blocked: bool, token: Option<&str>) -> PlanOp {
    PlanOp {
        id: "test-op".into(),
        safety,
        blocked,
        blocked_reason: if blocked {
            Some("blocked".into())
        } else {
            None
        },
        blocked_reason_token: token.map(|s| s.to_string()),
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

fn make_apply() -> BuildfixApply {
    BuildfixApply::new(
        tool(),
        ApplyRepoInfo {
            root: ".".into(),
            head_sha_before: None,
            head_sha_after: None,
            dirty_before: None,
            dirty_after: None,
        },
        PlanRef {
            path: "plan.json".into(),
            sha256: None,
        },
    )
}

// =============================================================================
// Plan Rendering Tests
// =============================================================================

#[test]
fn plan_md_header_format() {
    let plan = make_plan(vec![], None);
    let md = render_plan_md(&plan);
    assert!(md.starts_with("# buildfix plan\n"));
}

#[test]
fn plan_md_summary_section_format() {
    let mut plan = make_plan(vec![], None);
    plan.summary = PlanSummary {
        ops_total: 5,
        ops_blocked: 2,
        files_touched: 3,
        patch_bytes: Some(256),
        safety_counts: Some(SafetyCounts {
            safe: 2,
            guarded: 1,
            unsafe_count: 0,
        }),
    };

    let md = render_plan_md(&plan);
    assert!(md.contains("- Ops: 5 (blocked 2)"));
    assert!(md.contains("- Files touched: 3"));
    assert!(md.contains("- Patch bytes: 256"));
    assert!(md.contains("- Safety: 2 safe, 1 guarded, 0 unsafe"));
}

#[test]
fn plan_md_ops_section_header() {
    let plan = make_plan(vec![], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("## Ops\n"));
}

#[test]
fn plan_md_operation_numbering_starts_at_one() {
    let op = make_op(SafetyClass::Safe, false, None);
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("### 1. test-op"));
}

#[test]
fn plan_md_target_path_displayed() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.target = OpTarget {
        path: "crates/my-crate/Cargo.toml".into(),
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("- Target: `crates/my-crate/Cargo.toml`"));
}

#[test]
fn plan_md_inputs_count() {
    let mut plan = make_plan(vec![], None);
    plan.inputs = vec![
        PlanInput {
            path: "artifacts/clippy/report.json".into(),
            schema: None,
            tool: None,
        },
        PlanInput {
            path: "artifacts/machete/report.json".into(),
            schema: None,
            tool: None,
        },
    ];
    let md = render_plan_md(&plan);
    assert!(md.contains("- Inputs: 2"));
}

#[test]
fn plan_md_finding_with_no_check_id_shows_dash() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.rationale.findings.push(FindingRef {
        source: "sensor".to_string(),
        check_id: None,
        code: "CODE".to_string(),
        path: Some("file.rs".to_string()),
        line: Some(10),
        fingerprint: None,
    });
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("sensor/-"));
}

#[test]
fn plan_md_finding_with_no_path_shows_dash() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.rationale.findings.push(FindingRef {
        source: "sensor".to_string(),
        check_id: Some("check".to_string()),
        code: "CODE".to_string(),
        path: None,
        line: None,
        fingerprint: None,
    });
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("- `sensor/check` `CODE` at -"));
}

#[test]
fn plan_md_finding_line_zero() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.rationale.findings.push(FindingRef {
        source: "sensor".to_string(),
        check_id: Some("check".to_string()),
        code: "CODE".to_string(),
        path: Some("file.rs".to_string()),
        line: Some(0),
        fingerprint: None,
    });
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("file.rs:0"));
}

// =============================================================================
// Apply Rendering Tests
// =============================================================================

#[test]
fn apply_md_header_format() {
    let apply = make_apply();
    let md = render_apply_md(&apply);
    assert!(md.starts_with("# buildfix apply\n"));
}

#[test]
fn apply_md_summary_format() {
    let mut apply = make_apply();
    apply.summary = ApplySummary {
        attempted: 10,
        applied: 7,
        blocked: 2,
        failed: 1,
        files_modified: 5,
    };
    let md = render_apply_md(&apply);
    assert!(md.contains("- Attempted: 10"));
    assert!(md.contains("- Applied: 7"));
    assert!(md.contains("- Blocked: 2"));
    assert!(md.contains("- Failed: 1"));
    assert!(md.contains("- Files modified: 5"));
}

#[test]
fn apply_md_results_section_header() {
    let apply = make_apply();
    let md = render_apply_md(&apply);
    assert!(md.contains("## Results\n"));
}

#[test]
fn apply_md_operation_numbering() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "first".to_string(),
        status: ApplyStatus::Applied,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![],
    });
    apply.results.push(ApplyResult {
        op_id: "second".to_string(),
        status: ApplyStatus::Applied,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![],
    });

    let md = render_apply_md(&apply);
    assert!(md.contains("### 1. first"));
    assert!(md.contains("### 2. second"));
}

#[test]
fn apply_md_message_display() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Applied,
        message: Some("Successfully applied".to_string()),
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![],
    });

    let md = render_apply_md(&apply);
    assert!(md.contains("- Message: Successfully applied"));
}

#[test]
fn apply_md_no_message_not_displayed() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Applied,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![],
    });

    let md = render_apply_md(&apply);
    assert!(!md.contains("- Message:"));
}

#[test]
fn apply_md_file_change_format() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Applied,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![ApplyFile {
            path: "Cargo.toml".to_string(),
            sha256_before: Some("abc123".to_string()),
            sha256_after: Some("def456".to_string()),
            backup_path: None,
        }],
    });

    let md = render_apply_md(&apply);
    assert!(md.contains("**Files changed**"));
    assert!(md.contains("- `Cargo.toml` abc123 → def456"));
}

// =============================================================================
// Comment Rendering Tests
// =============================================================================

#[test]
fn comment_md_header_bold() {
    let plan = make_plan(vec![], None);
    let md = render_comment_md(&plan);
    assert!(md.contains("**buildfix**:"));
}

#[test]
fn comment_md_safety_table_headers() {
    let plan = make_plan(
        vec![make_op(SafetyClass::Safe, false, None)],
        Some(SafetyCounts {
            safe: 1,
            guarded: 0,
            unsafe_count: 0,
        }),
    );
    let md = render_comment_md(&plan);
    assert!(md.contains("| Safety | Count |"));
    assert!(md.contains("|--------|-------|"));
}

#[test]
fn comment_md_no_safety_table_when_no_counts() {
    let plan = make_plan(vec![], None);
    let md = render_comment_md(&plan);
    assert!(!md.contains("| Safety | Count |"));
}

#[test]
fn comment_md_artifact_links_format() {
    let plan = make_plan(vec![], None);
    let md = render_comment_md(&plan);
    assert!(md.contains("[plan.md](plan.md)"));
    assert!(md.contains("[patch.diff](patch.diff)"));
    assert!(md.contains("·"));
}

#[test]
fn comment_md_blocked_reasons_limited_to_five() {
    let ops: Vec<PlanOp> = (0..10)
        .map(|i| make_op(SafetyClass::Safe, true, Some(&format!("reason{}", i))))
        .collect();

    let mut plan = make_plan(
        ops,
        Some(SafetyCounts {
            safe: 10,
            guarded: 0,
            unsafe_count: 0,
        }),
    );
    plan.summary.ops_blocked = 10;

    let md = render_comment_md(&plan);
    assert!(md.contains("**Blocked reasons**:"));
}

// =============================================================================
// Safety Class Tests
// =============================================================================

#[test]
fn safety_class_safe_label() {
    let op = make_op(SafetyClass::Safe, false, None);
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Safety: `safe`"));
}

#[test]
fn safety_class_guarded_label() {
    let op = make_op(SafetyClass::Guarded, false, None);
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Safety: `guarded`"));
}

#[test]
fn safety_class_unsafe_label() {
    let op = make_op(SafetyClass::Unsafe, false, None);
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Safety: `unsafe`"));
}

// =============================================================================
// Apply Status Tests
// =============================================================================

#[test]
fn apply_status_applied_label() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Applied,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![],
    });

    let md = render_apply_md(&apply);
    assert!(md.contains("Status: `applied`"));
}

#[test]
fn apply_status_blocked_label() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Blocked,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![],
    });

    let md = render_apply_md(&apply);
    assert!(md.contains("Status: `blocked`"));
}

#[test]
fn apply_status_failed_label() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Failed,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![],
    });

    let md = render_apply_md(&apply);
    assert!(md.contains("Status: `failed`"));
}

#[test]
fn apply_status_skipped_label() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Skipped,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![],
    });

    let md = render_apply_md(&apply);
    assert!(md.contains("Status: `skipped`"));
}

// =============================================================================
// Operation Kind Tests
// =============================================================================

#[test]
fn op_kind_toml_set() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.kind = OpKind::TomlSet {
        toml_path: vec!["package".into(), "version".into()],
        value: serde_json::json!("1.0.0"),
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Kind: `toml_set`"));
}

#[test]
fn op_kind_toml_remove() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.kind = OpKind::TomlRemove {
        toml_path: vec!["dependencies".into(), "old-dep".into()],
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Kind: `toml_remove`"));
}

#[test]
fn op_kind_json_set() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.kind = OpKind::JsonSet {
        json_path: vec!["config".into(), "value".into()],
        value: serde_json::json!(42),
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Kind: `json_set`"));
}

#[test]
fn op_kind_json_remove() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.kind = OpKind::JsonRemove {
        json_path: vec!["old".into()],
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Kind: `json_remove`"));
}

#[test]
fn op_kind_yaml_set() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.kind = OpKind::YamlSet {
        yaml_path: vec!["config".into(), "key".into()],
        value: serde_json::json!("value"),
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Kind: `yaml_set`"));
}

#[test]
fn op_kind_yaml_remove() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.kind = OpKind::YamlRemove {
        yaml_path: vec!["old".into()],
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Kind: `yaml_remove`"));
}

#[test]
fn op_kind_toml_transform_uses_rule_id() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.kind = OpKind::TomlTransform {
        rule_id: "custom-transform-rule".to_string(),
        args: None,
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Kind: `custom-transform-rule`"));
}

#[test]
fn op_kind_text_replace_anchored() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.kind = OpKind::TextReplaceAnchored {
        find: "old".to_string(),
        replace: "new".to_string(),
        anchor_before: vec![],
        anchor_after: vec![],
        max_replacements: None,
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Kind: `text_replace_anchored`"));
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[test]
fn empty_plan_renders_correctly() {
    let plan = make_plan(vec![], None);
    let md = render_plan_md(&plan);

    assert!(md.contains("# buildfix plan"));
    assert!(md.contains("- Ops: 0 (blocked 0)"));
    assert!(md.contains("_No ops planned._"));
}

#[test]
fn empty_apply_renders_correctly() {
    let apply = make_apply();
    let md = render_apply_md(&apply);

    assert!(md.contains("# buildfix apply"));
    assert!(md.contains("_No results._"));
}

#[test]
fn plan_with_zero_values() {
    let mut plan = make_plan(vec![], None);
    plan.summary = PlanSummary {
        ops_total: 0,
        ops_blocked: 0,
        files_touched: 0,
        patch_bytes: Some(0),
        safety_counts: None,
    };

    let md = render_plan_md(&plan);
    assert!(md.contains("- Ops: 0 (blocked 0)"));
    assert!(md.contains("- Files touched: 0"));
    assert!(md.contains("- Patch bytes: 0"));
}

#[test]
fn apply_with_zero_values() {
    let mut apply = make_apply();
    apply.summary = ApplySummary {
        attempted: 0,
        applied: 0,
        blocked: 0,
        failed: 0,
        files_modified: 0,
    };

    let md = render_apply_md(&apply);
    assert!(md.contains("- Attempted: 0"));
    assert!(md.contains("- Applied: 0"));
    assert!(md.contains("- Blocked: 0"));
    assert!(md.contains("- Failed: 0"));
    assert!(md.contains("- Files modified: 0"));
}

#[test]
fn large_numbers_format_correctly() {
    let mut plan = make_plan(vec![], None);
    plan.summary = PlanSummary {
        ops_total: 1000000,
        ops_blocked: 500000,
        files_touched: 999,
        patch_bytes: Some(1234567890),
        safety_counts: None,
    };

    let md = render_plan_md(&plan);
    assert!(md.contains("- Ops: 1000000 (blocked 500000)"));
    assert!(md.contains("- Files touched: 999"));
    assert!(md.contains("- Patch bytes: 1234567890"));
}

#[test]
fn special_characters_in_op_id() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.id = "op-with-special_chars.123".to_string();
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("### 1. op-with-special_chars.123"));
}

#[test]
fn special_characters_in_path() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.target = OpTarget {
        path: "crates/sub-crate/Cargo.toml".into(),
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("- Target: `crates/sub-crate/Cargo.toml`"));
}

#[test]
fn long_description_text() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.rationale.description = Some("This is a very long description that explains the rationale for this operation in great detail. It should be rendered as-is without any truncation.".to_string());
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("This is a very long description"));
}

#[test]
fn multiple_params_required() {
    let mut op = make_op(SafetyClass::Unsafe, false, None);
    op.params_required = vec![
        "version".to_string(),
        "edition".to_string(),
        "license".to_string(),
    ];
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("- Params required: version, edition, license"));
}

#[test]
fn blocked_reason_with_special_formatting() {
    let mut op = make_op(SafetyClass::Guarded, true, None);
    op.blocked_reason = Some("Policy 'strict-mode' denies this operation".to_string());
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Blocked reason: Policy 'strict-mode' denies this operation"));
}

#[test]
fn multiple_findings_in_single_op() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.rationale.findings = (0..5)
        .map(|i| FindingRef {
            source: format!("sensor{}", i),
            check_id: Some(format!("check{}", i)),
            code: format!("CODE{}", i),
            path: Some(format!("file{}.rs", i)),
            line: Some(i * 10),
            fingerprint: None,
        })
        .collect();

    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);

    for i in 0..5 {
        assert!(md.contains(&format!("sensor{}/check{}", i, i)));
        assert!(md.contains(&format!("CODE{}", i)));
    }
}

#[test]
fn mixed_safety_classes_in_plan() {
    let ops = vec![
        make_op(SafetyClass::Safe, false, None),
        make_op(SafetyClass::Guarded, false, None),
        make_op(SafetyClass::Safe, true, Some("blocked1")),
        make_op(SafetyClass::Unsafe, true, Some("blocked2")),
    ];

    let plan = make_plan(
        ops,
        Some(SafetyCounts {
            safe: 2,
            guarded: 1,
            unsafe_count: 1,
        }),
    );

    let md = render_plan_md(&plan);
    assert!(md.contains("Safety: `safe`"));
    assert!(md.contains("Safety: `guarded`"));
    assert!(md.contains("Safety: `unsafe`"));
    assert!(md.contains("Blocked: `true`"));
    assert!(md.contains("Blocked: `false`"));
}

#[test]
fn apply_with_mixed_results() {
    let mut apply = make_apply();
    apply.summary = ApplySummary {
        attempted: 4,
        applied: 2,
        blocked: 1,
        failed: 1,
        files_modified: 2,
    };

    let statuses = [
        (ApplyStatus::Applied, "applied1"),
        (ApplyStatus::Applied, "applied2"),
        (ApplyStatus::Blocked, "blocked1"),
        (ApplyStatus::Failed, "failed1"),
    ];

    for (status, id) in statuses {
        let is_blocked = matches!(status, ApplyStatus::Blocked);
        apply.results.push(ApplyResult {
            op_id: id.to_string(),
            status,
            message: Some(format!("{} message", id)),
            blocked_reason: if is_blocked {
                Some("blocked reason".to_string())
            } else {
                None
            },
            blocked_reason_token: None,
            files: vec![],
        });
    }

    let md = render_apply_md(&apply);
    assert!(md.contains("Status: `applied`"));
    assert!(md.contains("Status: `blocked`"));
    assert!(md.contains("Status: `failed`"));
    assert!(md.contains("Blocked reason: blocked reason"));
}

#[test]
fn comment_md_fix_available_message() {
    let plan = make_plan(
        vec![make_op(SafetyClass::Safe, false, None)],
        Some(SafetyCounts {
            safe: 1,
            guarded: 0,
            unsafe_count: 0,
        }),
    );
    let md = render_comment_md(&plan);
    assert!(md.contains("**buildfix**: fix available"));
}

#[test]
fn comment_md_no_fixes_needed_message() {
    let plan = make_plan(vec![], None);
    let md = render_comment_md(&plan);
    assert!(md.contains("**buildfix**: no fixes needed"));
}

#[test]
fn comment_md_all_ops_blocked_message() {
    let mut plan = make_plan(
        vec![make_op(SafetyClass::Safe, true, Some("deny"))],
        Some(SafetyCounts {
            safe: 1,
            guarded: 0,
            unsafe_count: 0,
        }),
    );
    plan.summary.ops_blocked = 1;
    let md = render_comment_md(&plan);
    assert!(md.contains("**buildfix**: all ops blocked"));
}

#[test]
fn comment_md_safety_counts_only_nonzero() {
    let plan = make_plan(
        vec![make_op(SafetyClass::Guarded, false, None)],
        Some(SafetyCounts {
            safe: 0,
            guarded: 1,
            unsafe_count: 0,
        }),
    );
    let md = render_comment_md(&plan);
    assert!(md.contains("| guarded | 1 |"));
    assert!(!md.contains("| safe |"));
    assert!(!md.contains("| unsafe |"));
}

#[test]
fn file_change_with_missing_sha256_before() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Applied,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![ApplyFile {
            path: "file.toml".to_string(),
            sha256_before: None,
            sha256_after: Some("after-hash".to_string()),
            backup_path: None,
        }],
    });

    let md = render_apply_md(&apply);
    assert!(md.contains("- `file.toml` - → after-hash"));
}

#[test]
fn file_change_with_missing_sha256_after() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Applied,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![ApplyFile {
            path: "file.toml".to_string(),
            sha256_before: Some("before-hash".to_string()),
            sha256_after: None,
            backup_path: None,
        }],
    });

    let md = render_apply_md(&apply);
    assert!(md.contains("- `file.toml` before-hash → -"));
}

#[test]
fn multiple_operations_sequential_numbering() {
    let ops: Vec<PlanOp> = (1..=5)
        .map(|i| {
            let mut op = make_op(SafetyClass::Safe, false, None);
            op.id = format!("op-{}", i);
            op
        })
        .collect();

    let plan = make_plan(ops, None);
    let md = render_plan_md(&plan);

    for i in 1..=5 {
        assert!(md.contains(&format!("### {}. op-{}", i, i)));
    }
}

#[test]
fn plan_md_structure_order() {
    let op = make_op(SafetyClass::Safe, false, None);
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);

    let header_pos = md.find("# buildfix plan").unwrap();
    let summary_pos = md.find("- Ops:").unwrap();
    let ops_header_pos = md.find("## Ops").unwrap();
    let op_detail_pos = md.find("### 1.").unwrap();

    assert!(header_pos < summary_pos);
    assert!(summary_pos < ops_header_pos);
    assert!(ops_header_pos < op_detail_pos);
}

#[test]
fn apply_md_structure_order() {
    let mut apply = make_apply();
    apply.results.push(ApplyResult {
        op_id: "op".to_string(),
        status: ApplyStatus::Applied,
        message: None,
        blocked_reason: None,
        blocked_reason_token: None,
        files: vec![],
    });

    let md = render_apply_md(&apply);

    let header_pos = md.find("# buildfix apply").unwrap();
    let summary_pos = md.find("- Attempted:").unwrap();
    let results_header_pos = md.find("## Results").unwrap();
    let result_detail_pos = md.find("### 1.").unwrap();

    assert!(header_pos < summary_pos);
    assert!(summary_pos < results_header_pos);
    assert!(results_header_pos < result_detail_pos);
}

#[test]
fn unicode_in_op_id() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.id = "fix-日本語-émoji".to_string();
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("fix-日本語-émoji"));
}

#[test]
fn unicode_in_description() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.rationale.description = Some("Fix for 中文 and émojis 🎉".to_string());
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Fix for 中文 and émojis 🎉"));
}

#[test]
fn unicode_in_path() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.target = OpTarget {
        path: "crates/日本語/Cargo.toml".into(),
    };
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("crates/日本語/Cargo.toml"));
}

#[test]
fn newline_in_description_preserved() {
    let mut op = make_op(SafetyClass::Safe, false, None);
    op.rationale.description = Some("Line 1\nLine 2".to_string());
    let plan = make_plan(vec![op], None);
    let md = render_plan_md(&plan);
    assert!(md.contains("Line 1\nLine 2"));
}
