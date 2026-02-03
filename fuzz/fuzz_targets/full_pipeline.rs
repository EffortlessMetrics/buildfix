#![no_main]

//! Fuzz target for the full receipts -> plan -> apply pipeline.
//!
//! This fuzzes the entire pipeline with structured arbitrary input to ensure
//! all components work together gracefully with malformed data.

use libfuzzer_sys::fuzz_target;
use buildfix_types::ops::{DepPreserve, FixId, Operation, SafetyClass, TriggerKey};
use buildfix_types::plan::{
    BuildfixPlan, FindingRef, PlanInputs, PlanPolicySnapshot, PlanReceiptRef, PlanSummary,
    PlannedFix, Precondition,
};
use buildfix_types::receipt::{
    Counts, Finding, Location, ReceiptEnvelope, RunInfo, Severity, ToolInfo, Verdict,
    VerdictStatus,
};
use camino::Utf8PathBuf;

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
    plan_id: String,
    fixes_count: u8,
    fix_id: String,
    fix_title: String,
    safety: SafetyChoice,
    op_type: OpChoice,
    manifest: String,
    dep_name: String,
    version: String,
}

#[derive(Debug, arbitrary::Arbitrary)]
enum SafetyChoice {
    Safe,
    Guarded,
    Unsafe,
}

#[derive(Debug, arbitrary::Arbitrary)]
enum OpChoice {
    EnsureWorkspaceResolverV2,
    SetPackageRustVersion,
    EnsurePathDepHasVersion,
    UseWorkspaceDependency,
}

#[derive(Debug, arbitrary::Arbitrary)]
struct PolicyConfig {
    allow_patterns: Vec<String>,
    deny_patterns: Vec<String>,
    require_clean_hashes: bool,
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
        if let Ok(mut doc) = toml_str.parse::<toml_edit::DocumentMut>() {
            for fix in &plan.fixes {
                for op in &fix.operations {
                    apply_operation_to_doc(&mut doc, op);
                }
            }
            let _ = doc.to_string();
        }
    }

    // Phase 6: Test policy matching with glob patterns.
    for pattern in &input.policy.allow_patterns {
        let _ = glob_match(pattern, &input.structured_plan.fix_id);
    }
    for pattern in &input.policy.deny_patterns {
        let _ = glob_match(pattern, &input.structured_plan.fix_id);
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
    let manifest = Utf8PathBuf::from(&sp.manifest);

    let operation = match sp.op_type {
        OpChoice::EnsureWorkspaceResolverV2 => {
            Operation::EnsureWorkspaceResolverV2 { manifest }
        }
        OpChoice::SetPackageRustVersion => {
            Operation::SetPackageRustVersion {
                manifest,
                rust_version: sp.version.clone(),
            }
        }
        OpChoice::EnsurePathDepHasVersion => {
            Operation::EnsurePathDepHasVersion {
                manifest,
                toml_path: vec!["dependencies".to_string(), sp.dep_name.clone()],
                dep: sp.dep_name.clone(),
                dep_path: "../some-crate".to_string(),
                version: sp.version.clone(),
            }
        }
        OpChoice::UseWorkspaceDependency => {
            Operation::UseWorkspaceDependency {
                manifest,
                toml_path: vec!["dependencies".to_string(), sp.dep_name.clone()],
                dep: sp.dep_name.clone(),
                preserved: DepPreserve::default(),
            }
        }
    };

    let safety = match sp.safety {
        SafetyChoice::Safe => SafetyClass::Safe,
        SafetyChoice::Guarded => SafetyClass::Guarded,
        SafetyChoice::Unsafe => SafetyClass::Unsafe,
    };

    let mut fixes = Vec::new();
    for i in 0..sp.fixes_count.min(5) {
        fixes.push(PlannedFix {
            id: format!("{}-{}", sp.plan_id, i),
            fix_id: FixId::new(&sp.fix_id),
            safety,
            title: sp.fix_title.clone(),
            description: None,
            triggers: vec![FindingRef {
                trigger: TriggerKey::new("test-tool", None, None),
                message: None,
                location: None,
                fingerprint: None,
                data: None,
            }],
            operations: vec![operation.clone()],
            preconditions: vec![],
        });
    }

    BuildfixPlan {
        schema: "buildfix.plan.v1".to_string(),
        tool: ToolInfo {
            name: "buildfix-fuzz".to_string(),
            version: Some("0.0.0".to_string()),
            repo: None,
            commit: None,
        },
        run: RunInfo::default(),
        plan_id: sp.plan_id.clone(),
        policy: PlanPolicySnapshot {
            allow: policy.allow_patterns.clone(),
            deny: policy.deny_patterns.clone(),
            require_clean_hashes: policy.require_clean_hashes,
            caps: Default::default(),
        },
        inputs: PlanInputs {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
        },
        receipts: vec![PlanReceiptRef {
            sensor_id: "test-sensor".to_string(),
            report_path: Utf8PathBuf::from("artifacts/test-sensor/report.json"),
            schema: Some("test.v1".to_string()),
            tool_name: Some("test-tool".to_string()),
            parse_ok: true,
            error: None,
        }],
        summary: PlanSummary {
            fixes_total: fixes.len() as u64,
            safe: if safety == SafetyClass::Safe { fixes.len() as u64 } else { 0 },
            guarded: if safety == SafetyClass::Guarded { fixes.len() as u64 } else { 0 },
            unsafe_: if safety == SafetyClass::Unsafe { fixes.len() as u64 } else { 0 },
        },
        fixes,
        notes: vec![],
    }
}

/// Simple glob matching (mirrors buildfix-edit logic).
fn glob_match(pat: &str, text: &str) -> bool {
    let p = pat.as_bytes();
    let t = text.as_bytes();

    // Avoid excessive allocations for very long patterns.
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

/// Apply an operation to a TOML document.
fn apply_operation_to_doc(doc: &mut toml_edit::DocumentMut, op: &Operation) {
    use toml_edit::{value, InlineTable};

    match op {
        Operation::EnsureWorkspaceResolverV2 { .. } => {
            doc["workspace"]["resolver"] = value("2");
        }

        Operation::SetPackageRustVersion { rust_version, .. } => {
            doc["package"]["rust-version"] = value(rust_version.as_str());
        }

        Operation::EnsurePathDepHasVersion {
            toml_path,
            dep_path,
            version,
            ..
        } => {
            if let Some(dep_item) = get_dep_item_mut(doc, toml_path) {
                if let Some(inline) = dep_item.as_inline_table_mut() {
                    let current_path = inline.get("path").and_then(|v| v.as_str());
                    if current_path == Some(dep_path.as_str()) {
                        if inline.get("version").and_then(|v| v.as_str()).is_none() {
                            inline.insert("version", str_value(version));
                        }
                    }
                } else if let Some(tbl) = dep_item.as_table_mut() {
                    let current_path = tbl
                        .get("path")
                        .and_then(|i| i.as_value())
                        .and_then(|v| v.as_str());
                    if current_path == Some(dep_path.as_str()) {
                        if tbl
                            .get("version")
                            .and_then(|i| i.as_value())
                            .and_then(|v| v.as_str())
                            .is_none()
                        {
                            tbl["version"] = value(version.as_str());
                        }
                    }
                }
            }
        }

        Operation::UseWorkspaceDependency {
            toml_path,
            preserved,
            ..
        } => {
            if let Some(dep_item) = get_dep_item_mut(doc, toml_path) {
                let mut inline = InlineTable::new();
                inline.insert("workspace", bool_value(true));
                if let Some(pkg) = &preserved.package {
                    inline.insert("package", str_value(pkg));
                }
                if let Some(opt) = preserved.optional {
                    inline.insert("optional", bool_value(opt));
                }
                if let Some(df) = preserved.default_features {
                    inline.insert("default-features", bool_value(df));
                }
                if !preserved.features.is_empty() {
                    let mut arr = toml_edit::Array::new();
                    for f in &preserved.features {
                        arr.push(f.as_str());
                    }
                    inline.insert("features", value(arr).as_value().unwrap().clone());
                }
                *dep_item = value(inline);
            }
        }
    }
}

fn str_value(s: &str) -> toml_edit::Value {
    toml_edit::value(s).as_value().unwrap().clone()
}

fn bool_value(b: bool) -> toml_edit::Value {
    toml_edit::value(b).as_value().unwrap().clone()
}

fn get_dep_item_mut<'a>(
    doc: &'a mut toml_edit::DocumentMut,
    toml_path: &[String],
) -> Option<&'a mut toml_edit::Item> {
    if toml_path.len() < 2 {
        return None;
    }

    if toml_path[0] == "target" {
        if toml_path.len() < 4 {
            return None;
        }
        let cfg = &toml_path[1];
        let table_name = &toml_path[2];
        let dep = &toml_path[3];

        let target = doc.get_mut("target")?.as_table_mut()?;
        let cfg_tbl = target.get_mut(cfg)?.as_table_mut()?;
        let deps = cfg_tbl.get_mut(table_name)?.as_table_mut()?;
        return deps.get_mut(dep);
    }

    let table_name = &toml_path[0];
    let dep = &toml_path[1];
    let deps = doc.get_mut(table_name)?.as_table_mut()?;
    deps.get_mut(dep)
}
