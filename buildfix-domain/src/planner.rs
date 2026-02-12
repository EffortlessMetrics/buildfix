use crate::fixers;
use crate::ports::RepoView;
use anyhow::Context;
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::OpKind;
use buildfix_types::plan::{
    BuildfixPlan, FindingRef, PlanInput, PlanOp, PlanPolicy, PlanSummary, RepoInfo, SafetyCounts,
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

    let mut safe = 0u64;
    let mut guarded = 0u64;
    let mut unsafe_count = 0u64;
    for op in ops {
        match op.safety {
            buildfix_types::ops::SafetyClass::Safe => safe += 1,
            buildfix_types::ops::SafetyClass::Guarded => guarded += 1,
            buildfix_types::ops::SafetyClass::Unsafe => unsafe_count += 1,
        }
    }

    PlanSummary {
        ops_total,
        ops_blocked,
        files_touched,
        patch_bytes: None,
        safety_counts: Some(SafetyCounts {
            safe,
            guarded,
            unsafe_count,
        }),
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
            op.blocked_reason_token =
                Some(buildfix_types::plan::blocked_tokens::MISSING_PARAMS.to_string());
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
            op.blocked_reason_token =
                Some(buildfix_types::plan::blocked_tokens::DENYLIST.to_string());
            continue;
        }

        if !allow.is_empty() {
            let any_allow = allow
                .iter()
                .any(|pat| trigger_keys.iter().any(|k| glob_match(pat, k)));
            if !any_allow {
                op.blocked = true;
                op.blocked_reason = Some("not in allowlist".to_string());
                op.blocked_reason_token =
                    Some(buildfix_types::plan::blocked_tokens::ALLOWLIST_MISSING.to_string());
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
    let mut cap_token: Option<&str> = None;

    if let Some(max_ops) = cfg.max_ops {
        let total_ops = ops.len() as u64;
        if total_ops > max_ops {
            cap_reason = Some(format!(
                "caps exceeded: max_ops {} > {} allowed",
                total_ops, max_ops
            ));
            cap_token = Some(buildfix_types::plan::blocked_tokens::MAX_OPS);
        }
    }

    if cap_reason.is_none()
        && let Some(max_files) = cfg.max_files
    {
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
            cap_token = Some(buildfix_types::plan::blocked_tokens::MAX_FILES);
        }
    }

    if let Some(reason) = cap_reason {
        for op in ops.iter_mut() {
            op.blocked = true;
            op.blocked_reason = Some(reason.clone());
            op.blocked_reason_token = cap_token.map(|t| t.to_string());
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
                    fingerprint: f.fingerprint.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use buildfix_receipts::LoadedReceipt;
    use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
    use buildfix_types::plan::{PlanOp, Rationale, blocked_tokens};
    use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, RunInfo, ToolInfo, Verdict};
    use camino::Utf8PathBuf;
    use std::collections::HashMap;

    fn make_op(fix_key: &str, target: &str, kind: OpKind) -> PlanOp {
        PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: target.to_string(),
            },
            kind,
            rationale: Rationale {
                fix_key: fix_key.to_string(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        }
    }

    #[test]
    fn glob_match_handles_star_and_question() {
        assert!(glob_match("a*b", "ab"));
        assert!(glob_match("a*b", "acb"));
        assert!(!glob_match("a?b", "ab"));
        assert!(glob_match("a?b", "acb"));
        assert!(glob_match("cargo.*", "cargo.workspace_resolver_v2"));
        assert!(!glob_match("cargo.?1", "cargo.111"));
    }

    #[test]
    fn args_fingerprint_is_order_independent() {
        let mut map1 = serde_json::Map::new();
        map1.insert("b".to_string(), serde_json::json!(1));
        map1.insert("a".to_string(), serde_json::json!({"z": 2, "y": 3}));

        let mut map2 = serde_json::Map::new();
        map2.insert("a".to_string(), serde_json::json!({"y": 3, "z": 2}));
        map2.insert("b".to_string(), serde_json::json!(1));

        let fp1 = args_fingerprint(&Some(serde_json::Value::Object(map1)));
        let fp2 = args_fingerprint(&Some(serde_json::Value::Object(map2)));
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn deterministic_op_id_is_stable() {
        let mut args1 = serde_json::Map::new();
        args1.insert("b".to_string(), serde_json::json!(1));
        args1.insert("a".to_string(), serde_json::json!(2));

        let mut args2 = serde_json::Map::new();
        args2.insert("a".to_string(), serde_json::json!(2));
        args2.insert("b".to_string(), serde_json::json!(1));

        let op1 = make_op(
            "cargo.workspace_resolver_v2",
            "Cargo.toml",
            OpKind::TomlTransform {
                rule_id: "ensure_workspace_resolver_v2".to_string(),
                args: Some(serde_json::Value::Object(args1)),
            },
        );
        let op2 = make_op(
            "cargo.workspace_resolver_v2",
            "Cargo.toml",
            OpKind::TomlTransform {
                rule_id: "ensure_workspace_resolver_v2".to_string(),
                args: Some(serde_json::Value::Object(args2)),
            },
        );
        let op3 = make_op(
            "cargo.workspace_resolver_v2",
            "other/Cargo.toml",
            OpKind::TomlTransform {
                rule_id: "ensure_workspace_resolver_v2".to_string(),
                args: None,
            },
        );

        assert_eq!(deterministic_op_id(&op1), deterministic_op_id(&op2));
        assert_ne!(deterministic_op_id(&op1), deterministic_op_id(&op3));
    }

    #[test]
    fn apply_params_fills_transform_args() {
        let mut op = make_op(
            "cargo.path_dep_add_version",
            "Cargo.toml",
            OpKind::TomlTransform {
                rule_id: "ensure_path_dep_has_version".to_string(),
                args: None,
            },
        );
        op.params_required = vec!["version".to_string()];

        let mut params = HashMap::new();
        params.insert("version".to_string(), "1.2.3".to_string());

        let mut ops = vec![op];
        apply_params(&params, &mut ops);

        assert!(ops[0].params_required.is_empty());
        assert!(!ops[0].blocked);
        match &ops[0].kind {
            OpKind::TomlTransform { args: Some(v), .. } => {
                assert_eq!(v["version"], serde_json::json!("1.2.3"));
            }
            _ => panic!("expected toml transform with args"),
        }
    }

    #[test]
    fn apply_params_blocks_when_missing() {
        let mut op = make_op(
            "cargo.normalize_rust_version",
            "Cargo.toml",
            OpKind::TomlTransform {
                rule_id: "set_package_rust_version".to_string(),
                args: None,
            },
        );
        op.params_required = vec!["rust_version".to_string()];

        let mut ops = vec![op];
        apply_params(&HashMap::new(), &mut ops);

        assert!(ops[0].blocked);
        assert_eq!(
            ops[0].blocked_reason_token.as_deref(),
            Some(blocked_tokens::MISSING_PARAMS)
        );
    }

    #[test]
    fn apply_allow_deny_blocks_by_policy() {
        let mut ops = vec![make_op(
            "cargo.workspace_resolver_v2",
            "Cargo.toml",
            OpKind::TomlRemove {
                toml_path: vec!["workspace".to_string()],
            },
        )];
        apply_allow_deny(&[], &["cargo.*".to_string()], &mut ops);
        assert!(ops[0].blocked);
        assert_eq!(
            ops[0].blocked_reason_token.as_deref(),
            Some(blocked_tokens::DENYLIST)
        );

        let mut ops = vec![make_op(
            "cargo.workspace_resolver_v2",
            "Cargo.toml",
            OpKind::TomlRemove {
                toml_path: vec!["workspace".to_string()],
            },
        )];
        apply_allow_deny(&["depguard.*".to_string()], &[], &mut ops);
        assert!(ops[0].blocked);
        assert_eq!(
            ops[0].blocked_reason_token.as_deref(),
            Some(blocked_tokens::ALLOWLIST_MISSING)
        );
    }

    #[test]
    fn apply_allow_deny_allows_when_allowlist_matches() {
        let mut ops = vec![make_op(
            "cargo.workspace_resolver_v2",
            "Cargo.toml",
            OpKind::TomlRemove {
                toml_path: vec!["workspace".to_string()],
            },
        )];

        apply_allow_deny(&["cargo.*".to_string()], &[], &mut ops);
        assert!(!ops[0].blocked);
        assert!(ops[0].blocked_reason.is_none());
        assert!(ops[0].blocked_reason_token.is_none());
    }

    #[test]
    fn apply_allow_deny_does_not_override_existing_block() {
        let mut ops = vec![make_op(
            "cargo.workspace_resolver_v2",
            "Cargo.toml",
            OpKind::TomlRemove {
                toml_path: vec!["workspace".to_string()],
            },
        )];
        ops[0].blocked = true;
        ops[0].blocked_reason = Some("preblocked".to_string());
        ops[0].blocked_reason_token = Some("custom_token".to_string());

        apply_allow_deny(&["cargo.*".to_string()], &["cargo.*".to_string()], &mut ops);

        assert!(ops[0].blocked);
        assert_eq!(ops[0].blocked_reason.as_deref(), Some("preblocked"));
        assert_eq!(ops[0].blocked_reason_token.as_deref(), Some("custom_token"));
    }

    #[test]
    fn enforce_caps_blocks_all_ops() {
        let mut ops = vec![
            make_op(
                "cargo.workspace_resolver_v2",
                "Cargo.toml",
                OpKind::TomlRemove {
                    toml_path: vec!["workspace".to_string()],
                },
            ),
            make_op(
                "cargo.workspace_resolver_v2",
                "other/Cargo.toml",
                OpKind::TomlRemove {
                    toml_path: vec!["workspace".to_string()],
                },
            ),
        ];

        let cfg = PlannerConfig {
            max_ops: Some(1),
            ..Default::default()
        };
        enforce_caps(&cfg, &mut ops).expect("enforce caps");
        assert!(ops.iter().all(|op| op.blocked));
        assert_eq!(
            ops[0].blocked_reason_token.as_deref(),
            Some(blocked_tokens::MAX_OPS)
        );

        let mut ops = vec![
            make_op(
                "cargo.workspace_resolver_v2",
                "Cargo.toml",
                OpKind::TomlRemove {
                    toml_path: vec!["workspace".to_string()],
                },
            ),
            make_op(
                "cargo.workspace_resolver_v2",
                "other/Cargo.toml",
                OpKind::TomlRemove {
                    toml_path: vec!["workspace".to_string()],
                },
            ),
        ];
        let cfg = PlannerConfig {
            max_files: Some(1),
            ..Default::default()
        };
        enforce_caps(&cfg, &mut ops).expect("enforce caps");
        assert!(ops.iter().all(|op| op.blocked));
        assert_eq!(
            ops[0].blocked_reason_token.as_deref(),
            Some(blocked_tokens::MAX_FILES)
        );
    }

    #[test]
    fn receipt_set_filters_and_sorts_findings() {
        let receipt_a = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: ToolInfo {
                name: "builddiag".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("check".to_string()),
                code: Some("code".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from("b/Cargo.toml"),
                    line: Some(2),
                    column: None,
                }),
                fingerprint: None,
                data: None,
            }],
            capabilities: None,
            data: None,
        };

        let receipt_b = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: ToolInfo {
                name: "builddiag".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("check".to_string()),
                code: Some("code".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from("a/Cargo.toml"),
                    line: Some(1),
                    column: None,
                }),
                fingerprint: None,
                data: None,
            }],
            capabilities: None,
            data: None,
        };

        let loaded = vec![
            LoadedReceipt {
                path: Utf8PathBuf::from("artifacts/builddiag/report-b.json"),
                sensor_id: "builddiag".to_string(),
                receipt: Ok(receipt_a),
            },
            LoadedReceipt {
                path: Utf8PathBuf::from("artifacts/builddiag/report-a.json"),
                sensor_id: "builddiag".to_string(),
                receipt: Ok(receipt_b),
            },
        ];

        let set = ReceiptSet::from_loaded(&loaded);
        let findings = set.matching_findings(&["builddiag"], &["check"], &["code"]);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].path.as_deref(), Some("a/Cargo.toml"));
        assert_eq!(findings[1].path.as_deref(), Some("b/Cargo.toml"));
    }

    #[test]
    fn receipt_set_matches_when_filters_empty() {
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: ToolInfo {
                name: "builddiag".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("check".to_string()),
                code: Some("code".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from("Cargo.toml"),
                    line: Some(1),
                    column: None,
                }),
                fingerprint: None,
                data: None,
            }],
            capabilities: None,
            data: None,
        };

        let loaded = vec![LoadedReceipt {
            path: Utf8PathBuf::from("artifacts/builddiag/report.json"),
            sensor_id: "builddiag".to_string(),
            receipt: Ok(receipt),
        }];

        let set = ReceiptSet::from_loaded(&loaded);

        let all = set.matching_findings(&["builddiag"], &[], &[]);
        assert_eq!(all.len(), 1);

        let check_only = set.matching_findings(&["builddiag"], &["check"], &[]);
        assert_eq!(check_only.len(), 1);

        let code_only = set.matching_findings(&["builddiag"], &[], &["code"]);
        assert_eq!(code_only.len(), 1);

        let mismatch = set.matching_findings(&["builddiag"], &[], &["other"]);
        assert!(mismatch.is_empty());
    }
}
