use crate::fixers;
use crate::ports::RepoView;
use anyhow::Context;
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::{FixId, Operation, SafetyClass, TriggerKey};
use buildfix_types::plan::{
    BuildfixPlan, FindingRef, LocationRef, PlanInputs, PlanPolicySnapshot, PlanReceiptRef,
    PlanSummary, PlannedFix,
};
use buildfix_types::receipt::ToolInfo;
use camino::{Utf8Path, Utf8PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PlannerConfig {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub require_clean_hashes: bool,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            allow: vec![],
            deny: vec![],
            require_clean_hashes: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanContext {
    pub repo_root: Utf8PathBuf,
    pub artifacts_dir: Utf8PathBuf,
    pub config: PlannerConfig,
}

pub struct Planner {
    fixers: Vec<Box<dyn fixers::Fixer>>,
}

impl Planner {
    pub fn new() -> Self {
        Self {
            fixers: fixers::builtin_fixers(),
        }
    }

    pub fn with_fixers(fixers: Vec<Box<dyn fixers::Fixer>>) -> Self {
        Self { fixers }
    }

    pub fn plan(
        &self,
        ctx: &PlanContext,
        repo: &dyn RepoView,
        receipts: &[LoadedReceipt],
        tool: ToolInfo,
    ) -> anyhow::Result<BuildfixPlan> {
        let policy = PlanPolicySnapshot {
            allow: ctx.config.allow.clone(),
            deny: ctx.config.deny.clone(),
            require_clean_hashes: ctx.config.require_clean_hashes,
        };

        let inputs = PlanInputs {
            repo_root: ctx.repo_root.clone(),
            artifacts_dir: ctx.artifacts_dir.clone(),
        };

        let mut plan = BuildfixPlan::new(tool, inputs, policy);
        plan.receipts = receipts.iter().map(to_receipt_ref).collect();

        let receipt_set = ReceiptSet::from_loaded(receipts);

        let mut fixes: Vec<PlannedFix> = Vec::new();
        for fixer in &self.fixers {
            let mut f = fixer
                .plan(ctx, repo, &receipt_set)
                .with_context(|| "fixer.plan")?;
            fixes.append(&mut f);
        }

        // Deterministic ordering.
        fixes.sort_by(|a, b| stable_fix_sort_key(a).cmp(&stable_fix_sort_key(b)));

        // Deterministic ids.
        for fix in fixes.iter_mut() {
            if fix.id.trim().is_empty() {
                fix.id = deterministic_fix_id(&fix.fix_id, fix).to_string();
            }
        }

        plan.summary = summarize(&fixes);
        plan.fixes = fixes;
        Ok(plan)
    }
}

fn to_receipt_ref(r: &LoadedReceipt) -> PlanReceiptRef {
    match &r.receipt {
        Ok(env) => PlanReceiptRef {
            sensor_id: r.sensor_id.clone(),
            report_path: r.path.clone(),
            schema: Some(env.schema.clone()),
            tool_name: Some(env.tool.name.clone()),
            parse_ok: true,
            error: None,
        },
        Err(e) => PlanReceiptRef {
            sensor_id: r.sensor_id.clone(),
            report_path: r.path.clone(),
            schema: None,
            tool_name: None,
            parse_ok: false,
            error: Some(e.to_string()),
        },
    }
}

fn summarize(fixes: &[PlannedFix]) -> PlanSummary {
    let mut s = PlanSummary::default();
    s.fixes_total = fixes.len() as u64;
    for f in fixes {
        match f.safety {
            SafetyClass::Safe => s.safe += 1,
            SafetyClass::Guarded => s.guarded += 1,
            SafetyClass::Unsafe => s.unsafe_ += 1,
        }
    }
    s
}

/// Receipt access helpers used by fixers.
///
/// In tests, this can evolve into a richer index (finding fingerprints, etc.).
pub struct ReceiptSet {
    receipts: Vec<ReceiptRecord>,
}

#[derive(Debug, Clone)]
pub struct ReceiptRecord {
    pub sensor_id: String,
    pub path: Utf8PathBuf,
    pub envelope: buildfix_types::receipt::ReceiptEnvelope,
}

impl ReceiptSet {
    pub fn from_loaded(loaded: &[LoadedReceipt]) -> Self {
        let mut receipts = Vec::new();
        for r in loaded {
            if let Ok(env) = &r.receipt {
                receipts.push(ReceiptRecord {
                    sensor_id: r.sensor_id.clone(),
                    path: r.path.clone(),
                    envelope: env.clone(),
                });
            }
        }
        receipts.sort_by(|a, b| a.path.cmp(&b.path));
        Self { receipts }
    }

    pub fn matching_findings(
        &self,
        tool_prefixes: &[&str],
        check_ids: &[&str],
        codes: &[&str],
    ) -> Vec<FindingRef> {
        let mut out = Vec::new();

        for r in &self.receipts {
            let tool = r.envelope.tool.name.as_str();
            if !tool_prefixes.iter().any(|p| tool.starts_with(p)) {
                continue;
            }

            for f in &r.envelope.findings {
                let check_ok = if check_ids.is_empty() {
                    true
                } else {
                    f.check_id
                        .as_deref()
                        .map(|c| check_ids.contains(&c))
                        .unwrap_or(false)
                };

                let code_ok = if codes.is_empty() {
                    true
                } else {
                    f.code
                        .as_deref()
                        .map(|c| codes.contains(&c))
                        .unwrap_or(false)
                };

                if !check_ok || !code_ok {
                    continue;
                }

                out.push(FindingRef {
                    trigger: TriggerKey::new(tool.to_string(), f.check_id.clone(), f.code.clone()),
                    message: f.message.clone(),
                    location: f.location.as_ref().map(|loc| LocationRef {
                        path: loc.path.clone(),
                        line: loc.line,
                        column: loc.column,
                    }),
                    fingerprint: f.fingerprint.clone(),
                    data: f.data.clone(),
                });
            }
        }

        // Deterministic output order.
        out.sort_by(|a, b| stable_finding_key(a).cmp(&stable_finding_key(b)));
        out
    }
}

fn stable_finding_key(f: &FindingRef) -> String {
    let loc = f
        .location
        .as_ref()
        .map(|l| format!("{}:{}:{}", l.path, l.line.unwrap_or(0), l.column.unwrap_or(0)))
        .unwrap_or_else(|| "no_location".to_string());

    format!(
        "{}/{}/{}|{}",
        f.trigger.tool,
        f.trigger.check_id.clone().unwrap_or_default(),
        f.trigger.code.clone().unwrap_or_default(),
        loc
    )
}

fn stable_fix_sort_key(f: &PlannedFix) -> String {
    let manifest = f
        .operations
        .first()
        .map(|op| op.manifest().to_string())
        .unwrap_or_default();

    let op_key = f
        .operations
        .first()
        .map(op_sort_key)
        .unwrap_or_default();

    format!("{}|{}|{}", f.fix_id.0, manifest, op_key)
}

fn op_sort_key(op: &Operation) -> String {
    match op {
        Operation::EnsureWorkspaceResolverV2 { .. } => "resolver_v2".to_string(),
        Operation::EnsurePathDepHasVersion {
            toml_path, dep, ..
        } => format!("path_dep_version|{}|{}", dep, toml_path.join(".")),
        Operation::UseWorkspaceDependency {
            toml_path, dep, ..
        } => format!("workspace_dep|{}|{}", dep, toml_path.join(".")),
        Operation::SetPackageRustVersion { .. } => "msrv".to_string(),
    }
}

fn deterministic_fix_id(fix_id: &FixId, fix: &PlannedFix) -> Uuid {
    // Deterministic ID: v5(namespace, stable_key_bytes)
    const NAMESPACE: Uuid = Uuid::from_bytes([
        0x4b, 0x5d, 0x35, 0x58, 0x06, 0x58, 0x4c, 0x05, 0x8e, 0x8c, 0x0b, 0x1a, 0x44, 0x53,
        0x52, 0xd1,
    ]);

    let manifest = fix
        .operations
        .first()
        .map(|op| op.manifest().to_string())
        .unwrap_or_default();

    let op_key = fix
        .operations
        .first()
        .map(op_sort_key)
        .unwrap_or_default();

    let stable_key = format!("{}|{}|{}", fix_id.0, manifest, op_key);
    Uuid::new_v5(&NAMESPACE, stable_key.as_bytes())
}
