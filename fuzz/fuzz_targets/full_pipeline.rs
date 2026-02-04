#![no_main]

//! Fuzz target for the full receipts -> plan -> apply pipeline.
//!
//! This fuzzes the entire pipeline with structured arbitrary input to ensure
//! all components work together gracefully with malformed data.

use buildfix_edit::apply_op_to_content;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{BuildfixPlan, FindingRef, PlanOp, PlanPolicy, PlanSummary, Rationale, RepoInfo};
use buildfix_types::receipt::{
    Counts, Finding, Location, ReceiptEnvelope, RunInfo, Severity, ToolInfo, Verdict, VerdictStatus,
};
use camino::Utf8PathBuf;
use libfuzzer_sys::fuzz_target;

/// Structured fuzzing input for the full pipeline.
#[derive(Debug, arbitrary::Arbitrary)]
struct PipelineInput {
    /// Receipt data.
    receipt_json: Vec<u8>,
    /// Plan JSON data.
    plan_json: Vec<u8>,
    /// TOML file contents to apply operations to.
    toml_contents: Vec<u8>,
    /// Structured receipt to test serialization roundtrip.
    structured_receipt: StructuredReceipt,
    /// Structured plan to test serialization roundtrip.
    structured_plan: StructuredPlan,
    /// Policy configuration.
    policy: PolicyConfig,
}

#[derive(Debug, arbitrary::Arbitrary)]
struct StructuredReceipt {
    schema: String,
    tool_name: String,
    tool_version: Option<String>,
    verdict_status: VerdictChoice,
    findings_count: u8,
    finding_severity: SeverityChoice,
    finding_message: Option<String>,
    finding_path: String,
}

#[derive(Debug, arbitrary::Arbitrary)]
enum VerdictChoice {
    Pass,
    Warn,
    Fail,
    Unknown,
}

#[derive(Debug, arbitrary::Arbitrary)]
enum SeverityChoice {
    Info,
    Warning,
    Error,
}

#[derive(Debug, arbitrary::Arbitrary)]
struct StructuredPlan {
    ops_count: u8,
    fix_key: String,
    rule_id: String,
    target_path: String,
    safety: SafetyChoice,
}

#[derive(Debug, arbitrary::Arbitrary)]
enum SafetyChoice {
    Safe,
    Guarded,
    Unsafe,
}

#[derive(Debug, arbitrary::Arbitrary)]
struct PolicyConfig {
    allow_patterns: Vec<String>,
    deny_patterns: Vec<String>,
}

fuzz_target!(|input: PipelineInput| {
    // Phase 1: Parse raw receipt JSON.
    if let Ok(s) = std::str::from_utf8(&input.receipt_json) {
        let _ = serde_json::from_str::<ReceiptEnvelope>(s);
    }

    // Phase 2: Parse raw plan JSON.
    if let Ok(s) = std::str::from_utf8(&input.plan_json) {
        let _ = serde_json::from_str::<BuildfixPlan>(s);
    }

    // Phase 3: Build structured receipt and test serialization roundtrip.
    let receipt = build_receipt(&input.structured_receipt);
    if let Ok(json) = serde_json::to_string(&receipt) {
        let _ = serde_json::from_str::<ReceiptEnvelope>(&json);
    }

    // Phase 4: Build structured plan and test serialization roundtrip.
    let plan = build_plan(&input.structured_plan, &input.policy);
    if let Ok(json) = serde_json::to_string(&plan) {
        let _ = serde_json::from_str::<BuildfixPlan>(&json);
    }

    // Phase 5: Try to apply operations to TOML content.
    if let Ok(toml_str) = std::str::from_utf8(&input.toml_contents) {
        let mut current = toml_str.to_string();
        for op in &plan.ops {
            if let Ok(next) = apply_op_to_content(&current, &op.kind) {
                current = next;
            }
        }
        let _ = current;
    }

    // Phase 6: Test policy matching with glob patterns.
    for pattern in &input.policy.allow_patterns {
        let _ = glob_match(pattern, &input.structured_plan.fix_key);
    }
    for pattern in &input.policy.deny_patterns {
        let _ = glob_match(pattern, &input.structured_plan.fix_key);
    }
});

fn build_receipt(sr: &StructuredReceipt) -> ReceiptEnvelope {
    let mut findings = Vec::new();
    for _ in 0..sr.findings_count.min(10) {
        findings.push(Finding {
            severity: match sr.finding_severity {
                SeverityChoice::Info => Severity::Info,
                SeverityChoice::Warning => Severity::Warning,
                SeverityChoice::Error => Severity::Error,
            },
            check_id: None,
            code: None,
            message: sr.finding_message.clone(),
            location: Some(Location {
                path: Utf8PathBuf::from(&sr.finding_path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: None,
        });
    }

    ReceiptEnvelope {
        schema: sr.schema.clone(),
        tool: ToolInfo {
            name: sr.tool_name.clone(),
            version: sr.tool_version.clone(),
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        verdict: Verdict {
            status: match sr.verdict_status {
                VerdictChoice::Pass => VerdictStatus::Pass,
                VerdictChoice::Warn => VerdictStatus::Warn,
                VerdictChoice::Fail => VerdictStatus::Fail,
                VerdictChoice::Unknown => VerdictStatus::Unknown,
            },
            counts: Counts {
                findings: findings.len() as u64,
                errors: 0,
                warnings: 0,
            },
            reasons: vec![],
        },
        findings,
        data: None,
    }
}

fn build_plan(sp: &StructuredPlan, policy: &PolicyConfig) -> BuildfixPlan {
    let policy = PlanPolicy {
        allow: policy.allow_patterns.clone(),
        deny: policy.deny_patterns.clone(),
        allow_guarded: false,
        allow_unsafe: false,
        allow_dirty: false,
        max_ops: None,
        max_files: None,
        max_patch_bytes: None,
    };

    let repo = RepoInfo {
        root: ".".to_string(),
        head_sha: None,
        dirty: None,
    };

    let tool = ToolInfo {
        name: "buildfix-fuzz".to_string(),
        version: Some("0.0.0".to_string()),
        repo: None,
        commit: None,
    };

    let mut plan = BuildfixPlan::new(tool, repo, policy);

    let safety = match sp.safety {
        SafetyChoice::Safe => SafetyClass::Safe,
        SafetyChoice::Guarded => SafetyClass::Guarded,
        SafetyChoice::Unsafe => SafetyClass::Unsafe,
    };

    let count = sp.ops_count.min(5) as u64;
    for i in 0..count {
        plan.ops.push(PlanOp {
            id: format!("op-{}", i),
            safety,
            blocked: false,
            blocked_reason: None,
            target: OpTarget {
                path: sp.target_path.clone(),
            },
            kind: OpKind::TomlTransform {
                rule_id: sp.rule_id.clone(),
                args: None,
            },
            rationale: Rationale {
                fix_key: sp.fix_key.clone(),
                description: None,
                findings: vec![FindingRef {
                    source: "fuzz".to_string(),
                    check_id: None,
                    code: "-".to_string(),
                    path: None,
                    line: None,
                }],
            },
            params_required: vec![],
            preview: None,
        });
    }

    plan.summary = PlanSummary {
        ops_total: plan.ops.len() as u64,
        ops_blocked: 0,
        files_touched: plan
            .ops
            .iter()
            .map(|o| o.target.path.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as u64,
        patch_bytes: None,
    };

    plan
}

/// Simple glob matching (mirrors buildfix-domain logic).
fn glob_match(pat: &str, text: &str) -> bool {
    let p = pat.as_bytes();
    let t = text.as_bytes();

    if p.len() > 1000 || t.len() > 1000 {
        return false;
    }

    let mut dp = vec![vec![false; t.len() + 1]; p.len() + 1];
    dp[0][0] = true;

    for i in 1..=p.len() {
        if p[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        }
    }

    for i in 1..=p.len() {
        for j in 1..=t.len() {
            dp[i][j] = match p[i - 1] {
                b'*' => dp[i - 1][j] || dp[i][j - 1],
                b'?' => dp[i - 1][j - 1],
                c => dp[i - 1][j - 1] && c == t[j - 1],
            };
        }
    }

    dp[p.len()][t.len()]
}
