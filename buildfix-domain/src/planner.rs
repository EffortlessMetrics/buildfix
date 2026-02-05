use crate::fixers;
use crate::ports::RepoView;
use anyhow::Context;
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::OpKind;
use buildfix_types::plan::{
    BuildfixPlan, FindingRef, PlanInput, PlanOp, PlanPolicy, PlanSummary, RepoInfo,
};
use buildfix_types::receipt::ToolInfo;
use camino::Utf8PathBuf;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct PlannerConfig {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub allow_guarded: bool,
    pub allow_unsafe: bool,
    pub allow_dirty: bool,
    pub max_ops: Option<u64>,
    pub max_files: Option<u64>,
    pub max_patch_bytes: Option<u64>,
    pub params: HashMap<String, String>,
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

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
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
        let policy = PlanPolicy {
            allow: ctx.config.allow.clone(),
            deny: ctx.config.deny.clone(),
            allow_guarded: ctx.config.allow_guarded,
            allow_unsafe: ctx.config.allow_unsafe,
            allow_dirty: ctx.config.allow_dirty,
            max_ops: ctx.config.max_ops,
            max_files: ctx.config.max_files,
            max_patch_bytes: ctx.config.max_patch_bytes,
        };

        let repo_info = RepoInfo {
            root: ctx.repo_root.to_string(),
            head_sha: None,
            dirty: None,
        };

        let mut plan = BuildfixPlan::new(tool, repo_info, policy);
        plan.inputs = receipts.iter().map(to_plan_input).collect();

        let receipt_set = ReceiptSet::from_loaded(receipts);

        let mut ops: Vec<PlanOp> = Vec::new();
        for fixer in &self.fixers {
            let mut f = fixer
                .plan(ctx, repo, &receipt_set)
                .with_context(|| "fixer.plan")?;
            ops.append(&mut f);
        }

        // Deterministic ordering.
        ops.sort_by_key(stable_op_sort_key);

        // Deterministic ids.
        for op in ops.iter_mut() {
            if op.id.trim().is_empty() {
                op.id = deterministic_op_id(op).to_string();
            }
        }

        // Resolve params and apply policy.
        apply_params(&ctx.config.params, &mut ops);
        apply_allow_deny(&ctx.config.allow, &ctx.config.deny, &mut ops);

        // Enforce caps by blocking all ops if exceeded.
        enforce_caps(&ctx.config, &mut ops)?;

        plan.summary = summarize(&ops);
        plan.ops = ops;
        Ok(plan)
    }
}

fn to_plan_input(r: &LoadedReceipt) -> PlanInput {
    match &r.receipt {
        Ok(env) => PlanInput {
            path: r.path.to_string(),
            schema: Some(env.schema.clone()),
            tool: Some(env.tool.name.clone()),
        },
        Err(_) => PlanInput {
            path: r.path.to_string(),
            schema: None,
            tool: None,
        },
    }
}

fn summarize(ops: &[PlanOp]) -> PlanSummary {
    let ops_total = ops.len() as u64;
    let ops_blocked = ops.iter().filter(|o| o.blocked).count() as u64;
    let files_touched = ops
        .iter()
        .map(|o| o.target.path.as_str())
        .collect::<BTreeSet<_>>()
        .len() as u64;

    PlanSummary {
        ops_total,
        ops_blocked,
        files_touched,
        patch_bytes: None,
    }
}

fn apply_params(params: &HashMap<String, String>, ops: &mut [PlanOp]) {
    for op in ops {
        if op.params_required.is_empty() {
            continue;
        }

        let mut missing = Vec::new();
        let required = op.params_required.clone();
        for key in required {
            if let Some(value) = params.get(&key) {
                fill_op_param(op, &key, value);
            } else {
                missing.push(key);
            }
        }

        if missing.is_empty() {
            op.params_required.clear();
        } else {
            op.blocked = true;
            op.blocked_reason = Some(format!("missing params: {}", missing.join(", ")));
        }
    }
}

fn fill_op_param(op: &mut PlanOp, key: &str, value: &str) {
    let OpKind::TomlTransform { rule_id, args } = &mut op.kind else {
        return;
    };

    let mut map = match args.take() {
        Some(serde_json::Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };

    match (rule_id.as_str(), key) {
        ("set_package_rust_version", "rust_version") => {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        ("ensure_path_dep_has_version", "version") => {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
        _ => {
            map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    *args = Some(serde_json::Value::Object(map));
}

fn apply_allow_deny(allow: &[String], deny: &[String], ops: &mut [PlanOp]) {
    for op in ops {
        if op.blocked {
            continue;
        }

        let trigger_keys = op_fix_keys(op);

        if deny
            .iter()
            .any(|pat| trigger_keys.iter().any(|k| glob_match(pat, k)))
        {
            op.blocked = true;
            op.blocked_reason = Some("denied by policy".to_string());
            continue;
        }

        if !allow.is_empty() {
            let any_allow = allow
                .iter()
                .any(|pat| trigger_keys.iter().any(|k| glob_match(pat, k)));
            if !any_allow {
                op.blocked = true;
                op.blocked_reason = Some("not in allowlist".to_string());
            }
        }
    }
}

fn op_fix_keys(op: &PlanOp) -> Vec<String> {
    if op.rationale.findings.is_empty() {
        return vec![op.rationale.fix_key.clone()];
    }
    op.rationale
        .findings
        .iter()
        .map(fix_key_for_finding)
        .collect()
}

fn fix_key_for_finding(f: &FindingRef) -> String {
    let check = f.check_id.clone().unwrap_or_else(|| "-".to_string());
    format!("{}/{}/{}", f.source, check, f.code)
}

fn enforce_caps(cfg: &PlannerConfig, ops: &mut [PlanOp]) -> anyhow::Result<()> {
    let mut cap_reason: Option<String> = None;

    if let Some(max_ops) = cfg.max_ops {
        let total_ops = ops.len() as u64;
        if total_ops > max_ops {
            cap_reason = Some(format!(
                "caps exceeded: max_ops {} > {} allowed",
                total_ops, max_ops
            ));
        }
    }

    if cap_reason.is_none() {
        if let Some(max_files) = cfg.max_files {
            let files = ops
                .iter()
                .map(|o| o.target.path.as_str())
                .collect::<BTreeSet<_>>();
            let total_files = files.len() as u64;
            if total_files > max_files {
                cap_reason = Some(format!(
                    "caps exceeded: max_files {} > {} allowed",
                    total_files, max_files
                ));
            }
        }
    }

    if let Some(reason) = cap_reason {
        for op in ops.iter_mut() {
            op.blocked = true;
            op.blocked_reason = Some(reason.clone());
        }
    }

    Ok(())
}

fn glob_match(pat: &str, text: &str) -> bool {
    let p = pat.as_bytes();
    let t = text.as_bytes();
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

/// Receipt access helpers used by fixers.
pub struct ReceiptSet {
    receipts: Vec<ReceiptRecord>,
}

#[derive(Debug, Clone)]
pub struct ReceiptRecord {
    #[allow(dead_code)] // Useful for debugging/future use
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
                    source: tool.to_string(),
                    check_id: f.check_id.clone(),
                    code: f.code.clone().unwrap_or_else(|| "-".to_string()),
                    path: f.location.as_ref().map(|loc| loc.path.to_string()),
                    line: f.location.as_ref().and_then(|loc| loc.line),
                });
            }
        }

        out.sort_by_key(stable_finding_key);
        out
    }
}

fn stable_finding_key(f: &FindingRef) -> String {
    let loc = f
        .path
        .as_ref()
        .map(|p| format!("{}:{}", p, f.line.unwrap_or(0)))
        .unwrap_or_else(|| "no_location".to_string());

    format!(
        "{}/{}/{}|{}",
        f.source,
        f.check_id.clone().unwrap_or_default(),
        f.code,
        loc
    )
}

fn stable_op_sort_key(op: &PlanOp) -> String {
    let op_key = op_sort_key(op);
    format!("{}|{}|{}", op.rationale.fix_key, op.target.path, op_key)
}

fn op_sort_key(op: &PlanOp) -> String {
    match &op.kind {
        OpKind::TomlTransform { rule_id, args } => {
            format!("transform|{}|{}", rule_id, args_fingerprint(args))
        }
        OpKind::TomlSet { toml_path, .. } => format!("set|{}", toml_path.join(".")),
        OpKind::TomlRemove { toml_path } => format!("remove|{}", toml_path.join(".")),
    }
}

fn deterministic_op_id(op: &PlanOp) -> Uuid {
    // Deterministic ID: v5(namespace, stable_key_bytes)
    const NAMESPACE: Uuid = Uuid::from_bytes([
        0x4b, 0x5d, 0x35, 0x58, 0x06, 0x58, 0x4c, 0x05, 0x8e, 0x8c, 0x0b, 0x1a, 0x44, 0x53, 0x52,
        0xd1,
    ]);

    let rule_id = match &op.kind {
        OpKind::TomlTransform { rule_id, .. } => rule_id.as_str(),
        OpKind::TomlSet { .. } => "toml_set",
        OpKind::TomlRemove { .. } => "toml_remove",
    };

    let stable_key = format!(
        "{}|{}|{}|{}",
        op.rationale.fix_key,
        op.target.path,
        rule_id,
        args_fingerprint(match &op.kind {
            OpKind::TomlTransform { args, .. } => args,
            _ => &None,
        })
    );
    Uuid::new_v5(&NAMESPACE, stable_key.as_bytes())
}

fn args_fingerprint(args: &Option<serde_json::Value>) -> String {
    let Some(value) = args else {
        return "no_args".to_string();
    };
    let canonical = canonicalize_json(value);
    let s = serde_json::to_string(&canonical).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

fn canonicalize_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                if let Some(v) = map.get(&k) {
                    out.insert(k, canonicalize_json(v));
                }
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonicalize_json).collect())
        }
        other => other.clone(),
    }
}
