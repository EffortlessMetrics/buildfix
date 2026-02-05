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
