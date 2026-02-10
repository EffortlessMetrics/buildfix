//! Rendering helpers (markdown) for human-readable artifacts.

use buildfix_types::apply::{ApplyStatus, BuildfixApply};
use buildfix_types::ops::SafetyClass;
use buildfix_types::plan::BuildfixPlan;

pub fn render_plan_md(plan: &BuildfixPlan) -> String {
    let mut out = String::new();
    out.push_str("# buildfix plan\n\n");
    out.push_str(&format!(
        "- Ops: {} (blocked {})\n",
        plan.summary.ops_total, plan.summary.ops_blocked
    ));
    out.push_str(&format!(
        "- Files touched: {}\n",
        plan.summary.files_touched
    ));
    if let Some(bytes) = plan.summary.patch_bytes {
        out.push_str(&format!("- Patch bytes: {}\n", bytes));
    }
    if let Some(sc) = &plan.summary.safety_counts {
        out.push_str(&format!(
            "- Safety: {} safe, {} guarded, {} unsafe\n",
            sc.safe, sc.guarded, sc.unsafe_count
        ));
    }
    out.push_str(&format!("- Inputs: {}\n\n", plan.inputs.len()));

    out.push_str("## Ops\n\n");
    if plan.ops.is_empty() {
        out.push_str("_No ops planned._\n");
        return out;
    }

    for (i, op) in plan.ops.iter().enumerate() {
        out.push_str(&format!("### {}. {}\n\n", i + 1, op.id));
        out.push_str(&format!("- Safety: `{}`\n", safety_label(op.safety)));
        out.push_str(&format!("- Blocked: `{}`\n", op.blocked));
        out.push_str(&format!("- Target: `{}`\n", op.target.path));
        out.push_str(&format!(
            "- Kind: `{}`\n",
            match &op.kind {
                buildfix_types::ops::OpKind::TomlSet { .. } => "toml_set",
                buildfix_types::ops::OpKind::TomlRemove { .. } => "toml_remove",
                buildfix_types::ops::OpKind::TomlTransform { rule_id, .. } => rule_id,
            }
        ));
        if let Some(reason) = &op.blocked_reason {
            out.push_str(&format!("- Blocked reason: {}\n", reason));
        }
        if let Some(desc) = &op.rationale.description {
            out.push_str(&format!("\n{}\n", desc));
        }

        if !op.params_required.is_empty() {
            out.push_str(&format!(
                "- Params required: {}\n",
                op.params_required.join(", ")
            ));
        }

        if !op.rationale.findings.is_empty() {
            out.push_str("\n**Findings**\n\n");
            for f in &op.rationale.findings {
                let check = f.check_id.clone().unwrap_or_else(|| "-".to_string());
                let loc = f
                    .path
                    .as_ref()
                    .map(|p| format!("{}:{}", p, f.line.unwrap_or(0)))
                    .unwrap_or_else(|| "-".to_string());
                out.push_str(&format!(
                    "- `{}/{}` `{}` at {}\n",
                    f.source, check, f.code, loc
                ));
            }
        }

        out.push('\n');
    }

    out
}

pub fn render_apply_md(apply: &BuildfixApply) -> String {
    let mut out = String::new();
    out.push_str("# buildfix apply\n\n");
    out.push_str(&format!(
        "- Attempted: {}\n- Applied: {}\n- Blocked: {}\n- Failed: {}\n- Files modified: {}\n\n",
        apply.summary.attempted,
        apply.summary.applied,
        apply.summary.blocked,
        apply.summary.failed,
        apply.summary.files_modified
    ));

    out.push_str("## Results\n\n");
    if apply.results.is_empty() {
        out.push_str("_No results._\n");
        return out;
    }

    for (i, r) in apply.results.iter().enumerate() {
        out.push_str(&format!("### {}. {}\n\n", i + 1, r.op_id));
        out.push_str(&format!("- Status: `{}`\n", status_label(&r.status)));
        if let Some(msg) = &r.message {
            out.push_str(&format!("- Message: {}\n", msg));
        }
        if let Some(reason) = &r.blocked_reason {
            out.push_str(&format!("- Blocked reason: {}\n", reason));
        }
        if !r.files.is_empty() {
            out.push_str("\n**Files changed**\n\n");
            for fc in &r.files {
                let before = fc.sha256_before.as_deref().unwrap_or("-");
                let after = fc.sha256_after.as_deref().unwrap_or("-");
                out.push_str(&format!("- `{}` {} → {}\n", fc.path, before, after));
            }
        }
        out.push('\n');
    }

    out
}

/// Render a short cockpit-friendly comment summary.
pub fn render_comment_md(plan: &BuildfixPlan) -> String {
    let mut out = String::new();

    let ops_applicable = plan
        .summary
        .ops_total
        .saturating_sub(plan.summary.ops_blocked);
    let fix_available = ops_applicable > 0;

    if fix_available {
        out.push_str("**buildfix**: fix available\n\n");
    } else if plan.ops.is_empty() {
        out.push_str("**buildfix**: no fixes needed\n\n");
    } else {
        out.push_str("**buildfix**: all ops blocked\n\n");
    }

    if let Some(sc) = &plan.summary.safety_counts {
        out.push_str("| Safety | Count |\n|--------|-------|\n");
        if sc.safe > 0 {
            out.push_str(&format!("| safe | {} |\n", sc.safe));
        }
        if sc.guarded > 0 {
            out.push_str(&format!("| guarded | {} |\n", sc.guarded));
        }
        if sc.unsafe_count > 0 {
            out.push_str(&format!("| unsafe | {} |\n", sc.unsafe_count));
        }
        out.push('\n');
    }

    let tokens: std::collections::BTreeSet<&str> = plan
        .ops
        .iter()
        .filter_map(|o| o.blocked_reason_token.as_deref())
        .collect();
    if !tokens.is_empty() {
        out.push_str("**Blocked reasons**: ");
        let top: Vec<&str> = tokens.into_iter().take(5).collect();
        out.push_str(&top.join(", "));
        out.push_str("\n\n");
    }

    out.push_str("Artifacts: [plan.md](plan.md) · [patch.diff](patch.diff)\n");

    out
}

fn safety_label(s: SafetyClass) -> &'static str {
    match s {
        SafetyClass::Safe => "safe",
        SafetyClass::Guarded => "guarded",
        SafetyClass::Unsafe => "unsafe",
    }
}

fn status_label(s: &ApplyStatus) -> &'static str {
    match s {
        ApplyStatus::Applied => "applied",
        ApplyStatus::Blocked => "blocked",
        ApplyStatus::Failed => "failed",
        ApplyStatus::Skipped => "skipped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buildfix_types::apply::{
        ApplyFile, ApplyRepoInfo, ApplyResult, ApplyStatus, ApplySummary, BuildfixApply, PlanRef,
    };
    use buildfix_types::ops::{OpKind, OpTarget};
    use buildfix_types::plan::{
        FindingRef, PlanOp, PlanPolicy, PlanSummary, Rationale, RepoInfo, SafetyCounts,
    };
    use buildfix_types::receipt::ToolInfo;

    fn tool() -> ToolInfo {
        ToolInfo {
            name: "buildfix".into(),
            version: Some("0.0.0".into()),
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

    #[test]
    fn comment_md_no_ops() {
        let plan = make_plan(vec![], None);
        let md = render_comment_md(&plan);
        assert!(md.contains("no fixes needed"));
        assert!(md.contains("plan.md"));
        assert!(md.contains("patch.diff"));
    }

    #[test]
    fn comment_md_with_ops() {
        let plan = make_plan(
            vec![make_op(SafetyClass::Safe, false, None)],
            Some(SafetyCounts {
                safe: 1,
                guarded: 0,
                unsafe_count: 0,
            }),
        );
        let md = render_comment_md(&plan);
        assert!(md.contains("fix available"));
        assert!(md.contains("| safe | 1 |"));
    }

    #[test]
    fn comment_md_all_blocked() {
        let plan = make_plan(
            vec![make_op(SafetyClass::Safe, true, Some("denylist"))],
            Some(SafetyCounts {
                safe: 1,
                guarded: 0,
                unsafe_count: 0,
            }),
        );
        let md = render_comment_md(&plan);
        assert!(md.contains("all ops blocked"));
        assert!(md.contains("denylist"));
    }

    #[test]
    fn comment_md_artifact_links() {
        let plan = make_plan(vec![], None);
        let md = render_comment_md(&plan);
        assert!(md.contains("[plan.md](plan.md)"));
        assert!(md.contains("[patch.diff](patch.diff)"));
    }

    #[test]
    fn plan_md_includes_details_and_findings() {
        let mut op = make_op(SafetyClass::Guarded, true, Some("denylist"));
        op.blocked_reason = Some("denied by policy".to_string());
        op.rationale.description = Some("Normalize resolver".to_string());
        op.params_required = vec!["version".to_string()];
        op.rationale.findings.push(FindingRef {
            source: "builddiag".to_string(),
            check_id: Some("workspace.resolver_v2".to_string()),
            code: "RESOLVER".to_string(),
            path: Some("Cargo.toml".to_string()),
            line: Some(1),
            fingerprint: None,
        });

        let plan = make_plan(
            vec![op],
            Some(SafetyCounts {
                safe: 0,
                guarded: 1,
                unsafe_count: 0,
            }),
        );
        let md = render_plan_md(&plan);
        assert!(md.contains("# buildfix plan"));
        assert!(md.contains("Ops: 1 (blocked 1)"));
        assert!(md.contains("Safety: `guarded`"));
        assert!(md.contains("Blocked: `true`"));
        assert!(md.contains("Blocked reason: denied by policy"));
        assert!(md.contains("Normalize resolver"));
        assert!(md.contains("Params required: version"));
        assert!(md.contains("Findings"));
        assert!(md.contains("builddiag/workspace.resolver_v2"));
        assert!(md.contains("Cargo.toml:1"));
    }

    #[test]
    fn plan_md_handles_no_ops() {
        let plan = make_plan(vec![], None);
        let md = render_plan_md(&plan);
        assert!(md.contains("_No ops planned._"));
    }

    #[test]
    fn apply_md_includes_results_and_files() {
        let mut apply = BuildfixApply::new(
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
        );
        apply.summary = ApplySummary {
            attempted: 1,
            applied: 1,
            blocked: 0,
            failed: 0,
            files_modified: 1,
        };
        apply.results.push(ApplyResult {
            op_id: "op1".to_string(),
            status: ApplyStatus::Applied,
            message: Some("ok".to_string()),
            blocked_reason: None,
            blocked_reason_token: None,
            files: vec![ApplyFile {
                path: "Cargo.toml".to_string(),
                sha256_before: Some("before".to_string()),
                sha256_after: Some("after".to_string()),
                backup_path: None,
            }],
        });

        let md = render_apply_md(&apply);
        assert!(md.contains("# buildfix apply"));
        assert!(md.contains("Attempted: 1"));
        assert!(md.contains("Applied: 1"));
        assert!(md.contains("Status: `applied`"));
        assert!(md.contains("Message: ok"));
        assert!(md.contains("Files changed"));
        assert!(md.contains("Cargo.toml"));
        assert!(md.contains("before → after"));
    }

    #[test]
    fn apply_md_handles_no_results() {
        let apply = BuildfixApply::new(
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
        );
        let md = render_apply_md(&apply);
        assert!(md.contains("_No results._"));
    }

    #[test]
    fn plan_md_renders_remove_and_transform_kinds() {
        let mut remove_op = make_op(SafetyClass::Safe, false, None);
        remove_op.kind = OpKind::TomlRemove {
            toml_path: vec!["package".to_string(), "name".to_string()],
        };
        remove_op.id = "remove".to_string();

        let mut transform_op = make_op(SafetyClass::Safe, false, None);
        transform_op.kind = OpKind::TomlTransform {
            rule_id: "custom_rule".to_string(),
            args: None,
        };
        transform_op.id = "transform".to_string();

        let plan = make_plan(vec![remove_op, transform_op], None);
        let md = render_plan_md(&plan);
        assert!(md.contains("Kind: `toml_remove`"));
        assert!(md.contains("Kind: `custom_rule`"));
    }

    #[test]
    fn apply_md_renders_all_statuses_and_reasons() {
        let mut apply = BuildfixApply::new(
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
        );
        apply.summary = ApplySummary {
            attempted: 4,
            applied: 1,
            blocked: 1,
            failed: 1,
            files_modified: 1,
        };
        apply.results.push(ApplyResult {
            op_id: "applied".to_string(),
            status: ApplyStatus::Applied,
            message: None,
            blocked_reason: None,
            blocked_reason_token: None,
            files: vec![],
        });
        apply.results.push(ApplyResult {
            op_id: "blocked".to_string(),
            status: ApplyStatus::Blocked,
            message: Some("blocked".to_string()),
            blocked_reason: Some("reason".to_string()),
            blocked_reason_token: None,
            files: vec![],
        });
        apply.results.push(ApplyResult {
            op_id: "failed".to_string(),
            status: ApplyStatus::Failed,
            message: Some("failed".to_string()),
            blocked_reason: None,
            blocked_reason_token: None,
            files: vec![],
        });
        apply.results.push(ApplyResult {
            op_id: "skipped".to_string(),
            status: ApplyStatus::Skipped,
            message: Some("skipped".to_string()),
            blocked_reason: None,
            blocked_reason_token: None,
            files: vec![],
        });

        let md = render_apply_md(&apply);
        assert!(md.contains("Status: `applied`"));
        assert!(md.contains("Status: `blocked`"));
        assert!(md.contains("Status: `failed`"));
        assert!(md.contains("Status: `skipped`"));
        assert!(md.contains("Blocked reason: reason"));
    }
}
