//! Reusable domain policy helpers and deterministic plan-op utilities.

use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use buildfix_fixer_api::PlannerConfig;
use buildfix_types::ops::OpKind;
use buildfix_types::plan::{PlanOp, blocked_tokens};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Apply all planner-level policy and deterministic-normalization passes.
///
/// This is the preferred crate-level entrypoint for `buildfix-domain` policy
/// application, combining ordering, op-id generation, parameter filling,
/// allow/deny filtering, and cap enforcement in a single call.
pub fn apply_plan_policy(cfg: &PlannerConfig, ops: &mut [PlanOp]) -> Result<()> {
    // Deterministic ordering.
    ops.sort_by_key(stable_op_sort_key);

    // Deterministic ids.
    for op in ops.iter_mut() {
        if op.id.trim().is_empty() {
            op.id = deterministic_op_id(op).to_string();
        }
    }

    // Resolve params and apply policy gates.
    apply_params(&cfg.params, ops);
    apply_allow_deny(&cfg.allow, &cfg.deny, ops);

    // Enforce caps by blocking all ops when exceeded.
    enforce_caps(cfg, ops)?;

    Ok(())
}

/// Apply explicit user parameters to operations that require them.
///
/// Missing parameters leave the operation blocked with `MISSING_PARAMS`.
pub fn apply_params(params: &HashMap<String, String>, ops: &mut [PlanOp]) {
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
            op.blocked_reason_token = Some(blocked_tokens::MISSING_PARAMS.to_string());
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
        ("set_package_license", "license") => {
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

/// Apply allowlist/denylist policy gates on operations.
///
/// Existing blocked operations are preserved.
pub fn apply_allow_deny(allow: &[String], deny: &[String], ops: &mut [PlanOp]) {
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
            op.blocked_reason_token = Some(blocked_tokens::DENYLIST.to_string());
            continue;
        }

        if !allow.is_empty()
            && !allow
                .iter()
                .any(|pat| trigger_keys.iter().any(|k| glob_match(pat, k)))
        {
            op.blocked = true;
            op.blocked_reason = Some("not in allowlist".to_string());
            op.blocked_reason_token = Some(blocked_tokens::ALLOWLIST_MISSING.to_string());
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
        .map(|f| {
            let check = f.check_id.clone().unwrap_or_else(|| "-".to_string());
            format!("{}/{}/{}", f.source, check, f.code)
        })
        .collect()
}

/// Enforce planning caps (max ops and max files).
///
/// Caps are blocking all operations when exceeded.
pub fn enforce_caps(cfg: &PlannerConfig, ops: &mut [PlanOp]) -> Result<()> {
    let mut cap_reason: Option<String> = None;
    let mut cap_token: Option<&str> = None;

    if let Some(max_ops) = cfg.max_ops {
        let total_ops = ops.len() as u64;
        if total_ops > max_ops {
            cap_reason = Some(format!(
                "caps exceeded: max_ops {} > {} allowed",
                total_ops, max_ops
            ));
            cap_token = Some(blocked_tokens::MAX_OPS);
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
            cap_token = Some(blocked_tokens::MAX_FILES);
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

/// Stable sort key for deterministic plan operation ordering.
pub fn stable_op_sort_key(op: &PlanOp) -> String {
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
        OpKind::JsonSet { json_path, value } => format!(
            "json_set|{}|{}",
            json_path.join("."),
            args_fingerprint(&Some(value.clone()))
        ),
        OpKind::JsonRemove { json_path } => format!("json_remove|{}", json_path.join(".")),
        OpKind::YamlSet { yaml_path, value } => format!(
            "yaml_set|{}|{}",
            yaml_path.join("."),
            args_fingerprint(&Some(value.clone()))
        ),
        OpKind::YamlRemove { yaml_path } => format!("yaml_remove|{}", yaml_path.join(".")),
        OpKind::TextReplaceAnchored {
            find,
            replace,
            anchor_before,
            anchor_after,
            max_replacements,
        } => format!(
            "text_replace_anchored|{}|{}|{}|{}|{}",
            find,
            replace,
            anchor_before.join("\x1f"),
            anchor_after.join("\x1f"),
            max_replacements
                .map(|n| n.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
    }
}

/// Deterministic plan-op ID based on fix key, target path, rule kind and args.
pub fn deterministic_op_id(op: &PlanOp) -> Uuid {
    // Deterministic ID: v5(namespace, stable_key_bytes)
    const NAMESPACE: Uuid = Uuid::from_bytes([
        0x4b, 0x5d, 0x35, 0x58, 0x06, 0x58, 0x4c, 0x05, 0x8e, 0x8c, 0x0b, 0x1a, 0x44, 0x53, 0x52,
        0xd1,
    ]);

    let rule_id = match &op.kind {
        OpKind::TomlTransform { rule_id, .. } => rule_id.as_str(),
        OpKind::TomlSet { .. } => "toml_set",
        OpKind::TomlRemove { .. } => "toml_remove",
        OpKind::JsonSet { .. } => "json_set",
        OpKind::JsonRemove { .. } => "json_remove",
        OpKind::YamlSet { .. } => "yaml_set",
        OpKind::YamlRemove { .. } => "yaml_remove",
        OpKind::TextReplaceAnchored { .. } => "text_replace_anchored",
    };

    let kind_fingerprint = match &op.kind {
        OpKind::TomlTransform { args, .. } => args_fingerprint(args),
        OpKind::JsonSet { json_path, value } => args_fingerprint(&Some(serde_json::json!({
            "json_path": json_path,
            "value": value,
        }))),
        OpKind::JsonRemove { json_path } => args_fingerprint(&Some(serde_json::json!({
            "json_path": json_path,
        }))),
        OpKind::YamlSet { yaml_path, value } => args_fingerprint(&Some(serde_json::json!({
            "yaml_path": yaml_path,
            "value": value,
        }))),
        OpKind::YamlRemove { yaml_path } => args_fingerprint(&Some(serde_json::json!({
            "yaml_path": yaml_path,
        }))),
        OpKind::TextReplaceAnchored {
            find,
            replace,
            anchor_before,
            anchor_after,
            max_replacements,
        } => args_fingerprint(&Some(serde_json::json!({
            "find": find,
            "replace": replace,
            "anchor_before": anchor_before,
            "anchor_after": anchor_after,
            "max_replacements": max_replacements,
        }))),
        _ => args_fingerprint(&None),
    };

    let stable_key = format!(
        "{}|{}|{}|{}",
        op.rationale.fix_key, op.target.path, rule_id, kind_fingerprint
    );
    Uuid::new_v5(&NAMESPACE, stable_key.as_bytes())
}

/// Fingerprint arbitrary JSON with deterministic key order.
pub fn args_fingerprint(args: &Option<serde_json::Value>) -> String {
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

/// Lightweight wildcard matcher for policy keys.
///
/// Supports `*` and `?`.
pub fn glob_match(pat: &str, text: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_toml_plan_op(path: &str, rule_id: &str, fix_key: &str) -> buildfix_types::plan::PlanOp {
        buildfix_types::plan::PlanOp {
            id: String::new(),
            safety: buildfix_types::ops::SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: buildfix_types::ops::OpTarget {
                path: path.to_string(),
            },
            kind: buildfix_types::ops::OpKind::TomlTransform {
                rule_id: rule_id.to_string(),
                args: Some(serde_json::json!({
                    "version": "1.0",
                })),
            },
            rationale: buildfix_types::plan::Rationale {
                fix_key: fix_key.to_string(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        }
    }

    #[test]
    fn apply_plan_policy_assigns_ids_and_blocks_on_caps() {
        let mut ops = vec![
            make_toml_plan_op(
                "b/Cargo.toml",
                "set_package_rust_version",
                "cargo.normalize_rust_version",
            ),
            make_toml_plan_op(
                "a/Cargo.toml",
                "set_package_rust_version",
                "cargo.normalize_rust_version",
            ),
        ];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: Some(1),
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        apply_plan_policy(&cfg, &mut ops).expect("apply policy");

        assert!(ops.iter().all(|op| !op.id.is_empty()));
        assert_eq!(ops[0].target.path, "a/Cargo.toml");
        assert_eq!(ops[1].target.path, "b/Cargo.toml");
        assert!(ops.iter().all(|op| op.blocked));
        assert_eq!(
            ops[0].blocked_reason_token.as_deref(),
            Some(blocked_tokens::MAX_OPS)
        );
    }

    #[test]
    fn apply_plan_policy_applies_params_and_allow_policy() {
        let op = buildfix_types::plan::PlanOp {
            id: String::new(),
            safety: buildfix_types::ops::SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: buildfix_types::ops::OpTarget {
                path: "a/Cargo.toml".into(),
            },
            kind: buildfix_types::ops::OpKind::TomlTransform {
                rule_id: "set_package_license".into(),
                args: None,
            },
            rationale: buildfix_types::plan::Rationale {
                fix_key: "cargo.normalize_license".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec!["license".to_string()],
            preview: None,
        };

        let mut ops = vec![op];
        let cfg = PlannerConfig {
            allow: vec!["cargo.*".into()],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: None,
            max_files: None,
            max_patch_bytes: None,
            params: {
                let mut map = HashMap::new();
                map.insert("license".to_string(), "MIT".to_string());
                map
            },
        };

        apply_plan_policy(&cfg, &mut ops).expect("apply policy");

        match &ops[0].kind {
            buildfix_types::ops::OpKind::TomlTransform {
                args: Some(value), ..
            } => {
                assert_eq!(value["license"], serde_json::json!("MIT"));
            }
            _ => panic!("expected toml transform"),
        }

        assert!(ops[0].params_required.is_empty());
        assert!(!ops[0].blocked);
        assert!(ops[0].blocked_reason.is_none());
    }

    #[test]
    fn glob_match_handles_wildcards() {
        assert!(glob_match("a*b", "acb"));
        assert!(!glob_match("a?b", "ab"));
    }

    #[test]
    fn stable_ids_and_fingerprint_are_consistent() {
        let _op = serde_json::json!({
            "rationale": {
                "fix_key": "cargo.workspace_resolver_v2",
                "findings": []
            },
            "target": { "path": "Cargo.toml" },
            "kind": {
                "type": "toml_transform",
                "rule_id": "ensure_workspace_resolver_v2",
                "args": {
                    "a": 1,
                    "b": 2,
                },
            }
        });

        let op1 = buildfix_types::plan::PlanOp {
            id: "".into(),
            safety: buildfix_types::ops::SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: buildfix_types::ops::OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: buildfix_types::ops::OpKind::TomlTransform {
                rule_id: "ensure_workspace_resolver_v2".into(),
                args: Some(serde_json::json!({
                    "a": 1,
                    "b": 2,
                })),
            },
            rationale: buildfix_types::plan::Rationale {
                fix_key: "cargo.workspace_resolver_v2".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        };

        let mut map1 = serde_json::Map::new();
        map1.insert(
            "b".to_string(),
            serde_json::Value::Number(serde_json::Number::from(1)),
        );
        map1.insert(
            "a".to_string(),
            serde_json::Value::Number(serde_json::Number::from(2)),
        );

        let mut map2 = serde_json::Map::new();
        map2.insert(
            "a".to_string(),
            serde_json::Value::Number(serde_json::Number::from(2)),
        );
        map2.insert(
            "b".to_string(),
            serde_json::Value::Number(serde_json::Number::from(1)),
        );

        assert_eq!(
            args_fingerprint(&Some(serde_json::Value::Object(map1))),
            args_fingerprint(&Some(serde_json::Value::Object(map2)))
        );
        assert!(op1.id.is_empty());
        let other = buildfix_types::plan::PlanOp { ..op1.clone() };
        assert_eq!(deterministic_op_id(&op1), deterministic_op_id(&other));
    }

    #[test]
    fn policy_limits_block_all_ops_when_exceeded() {
        let mut ops = vec![
            buildfix_types::plan::PlanOp {
                id: String::new(),
                safety: buildfix_types::ops::SafetyClass::Safe,
                blocked: false,
                blocked_reason: None,
                blocked_reason_token: None,
                target: buildfix_types::ops::OpTarget { path: "a".into() },
                kind: buildfix_types::ops::OpKind::TomlTransform {
                    rule_id: "set_package_rust_version".into(),
                    args: None,
                },
                rationale: buildfix_types::plan::Rationale {
                    fix_key: "cargo.normalize_rust_version".into(),
                    description: None,
                    findings: vec![],
                },
                params_required: vec![],
                preview: None,
            },
            buildfix_types::plan::PlanOp {
                id: String::new(),
                safety: buildfix_types::ops::SafetyClass::Safe,
                blocked: false,
                blocked_reason: None,
                blocked_reason_token: None,
                target: buildfix_types::ops::OpTarget { path: "b".into() },
                kind: buildfix_types::ops::OpKind::TomlTransform {
                    rule_id: "set_package_rust_version".into(),
                    args: None,
                },
                rationale: buildfix_types::plan::Rationale {
                    fix_key: "cargo.normalize_rust_version".into(),
                    description: None,
                    findings: vec![],
                },
                params_required: vec![],
                preview: None,
            },
        ];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: Some(1),
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        enforce_caps(&cfg, &mut ops).expect("caps");
        assert!(ops.iter().all(|op| op.blocked));
    }
}
