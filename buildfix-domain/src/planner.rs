use crate::fixers;
use crate::ports::RepoView;
use anyhow::Context;
use buildfix_domain_policy::apply_plan_policy;
#[cfg(test)]
use buildfix_domain_policy::{
    apply_allow_deny, apply_params, args_fingerprint, deterministic_op_id, enforce_caps, glob_match,
};
#[cfg(test)]
use buildfix_fixer_api::PlannerConfig;
use buildfix_fixer_api::{PlanContext, ReceiptSet};
use buildfix_receipts::LoadedReceipt;
use buildfix_types::plan::{
    BuildfixPlan, PlanInput, PlanOp, PlanPolicy, PlanSummary, RepoInfo, SafetyCounts,
};
use buildfix_types::receipt::ToolInfo;
use std::collections::BTreeSet;

pub struct Planner {
    fixers: Vec<Box<dyn buildfix_fixer_api::Fixer>>,
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

    pub fn with_fixers(fixers: Vec<Box<dyn buildfix_fixer_api::Fixer>>) -> Self {
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

        apply_plan_policy(&ctx.config, &mut ops)?;

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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
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
