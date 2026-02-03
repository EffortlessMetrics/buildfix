//! Rendering helpers (markdown) for human-readable artifacts.

use buildfix_types::apply::{ApplyStatus, BuildfixApply};
use buildfix_types::ops::SafetyClass;
use buildfix_types::plan::BuildfixPlan;

pub fn render_plan_md(plan: &BuildfixPlan) -> String {
    let mut out = String::new();
    out.push_str("# buildfix plan\n\n");
    out.push_str(&format!("Plan id: `{}`\n\n", plan.plan_id));
    out.push_str(&format!(
        "- Fixes: {} (safe {}, guarded {}, unsafe {})\n",
        plan.summary.fixes_total, plan.summary.safe, plan.summary.guarded, plan.summary.unsafe_
    ));
    out.push_str(&format!("- Receipts: {}\n\n", plan.receipts.len()));

    out.push_str("## Fixes\n\n");
    if plan.fixes.is_empty() {
        out.push_str("_No fixes planned._\n");
        return out;
    }

    for (i, fix) in plan.fixes.iter().enumerate() {
        out.push_str(&format!("### {}. {}\n\n", i + 1, fix.title));
        out.push_str(&format!("- Fix id: `{}`\n", fix.fix_id.0));
        out.push_str(&format!("- Safety: `{}`\n", safety_label(fix.safety)));
        out.push_str(&format!("- Operations: {}\n", fix.operations.len()));
        if let Some(desc) = &fix.description {
            out.push_str(&format!("\n{}\n", desc));
        }

        if !fix.triggers.is_empty() {
            out.push_str("\n**Triggers**\n\n");
            for t in &fix.triggers {
                let check = t
                    .trigger
                    .check_id
                    .clone()
                    .unwrap_or_else(|| "-".to_string());
                let code = t.trigger.code.clone().unwrap_or_else(|| "-".to_string());
                let loc = t
                    .location
                    .as_ref()
                    .map(|l| format!("{}:{}", l.path, l.line.unwrap_or(0)))
                    .unwrap_or_else(|| "-".to_string());
                out.push_str(&format!(
                    "- `{}/{}` `{}` at {}\n",
                    t.trigger.tool, check, code, loc
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
    out.push_str(&format!("Plan id: `{}`\n\n", apply.plan_id));
    out.push_str(&format!(
        "- Attempted: {}\n- Applied: {}\n- Skipped: {}\n- Failed: {}\n\n",
        apply.summary.attempted, apply.summary.applied, apply.summary.skipped, apply.summary.failed
    ));

    out.push_str("## Results\n\n");
    if apply.results.is_empty() {
        out.push_str("_No results._\n");
        return out;
    }

    for (i, r) in apply.results.iter().enumerate() {
        out.push_str(&format!("### {}. {}\n\n", i + 1, r.title));
        out.push_str(&format!("- Fix id: `{}`\n", r.fix_id.0));
        out.push_str(&format!("- Safety: `{}`\n", safety_label(r.safety)));
        out.push_str(&format!("- Status: `{}`\n", status_label(&r.status)));
        if let Some(msg) = &r.message {
            out.push_str(&format!("- Message: {}\n", msg));
        }
        if !r.files_changed.is_empty() {
            out.push_str("\n**Files changed**\n\n");
            for fc in &r.files_changed {
                out.push_str(&format!(
                    "- `{}` {} → {}\n",
                    fc.path, fc.before_sha256, fc.after_sha256
                ));
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
        ApplyStatus::Skipped => "skipped",
        ApplyStatus::Failed => "failed",
    }
}
