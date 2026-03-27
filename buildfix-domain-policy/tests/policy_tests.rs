//! Comprehensive unit tests for buildfix-domain-policy.
//!
//! This test module covers:
//! - Policy evaluation (allow/deny decisions)
//! - Caps handling (max_ops, max_files)
//! - Edge cases in policy matching
//! - Helper functions (glob_match, deterministic IDs, fingerprints)

use std::collections::HashMap;

use buildfix_domain_policy::{
    apply_allow_deny, apply_params, apply_plan_policy, args_fingerprint, deterministic_op_id,
    enforce_caps, glob_match, stable_op_sort_key,
};
use buildfix_fixer_api::PlannerConfig;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{PlanOp, Rationale, blocked_tokens};

/// Helper to create a minimal PlanOp for testing.
fn make_plan_op(path: &str, rule_id: &str, fix_key: &str) -> PlanOp {
    PlanOp {
        id: String::new(),
        safety: SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        blocked_reason_token: None,
        target: OpTarget {
            path: path.to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: rule_id.to_string(),
            args: Some(serde_json::json!({ "version": "1.0" })),
        },
        rationale: Rationale {
            fix_key: fix_key.to_string(),
            description: None,
            findings: vec![],
        },
        params_required: vec![],
        preview: None,
    }
}

/// Helper to create a PlanOp with findings for testing finding-based fix keys.
fn make_plan_op_with_findings(
    path: &str,
    rule_id: &str,
    fix_key: &str,
    findings: Vec<(String, Option<String>, String)>,
) -> PlanOp {
    use buildfix_types::plan::FindingRef;
    PlanOp {
        id: String::new(),
        safety: SafetyClass::Safe,
        blocked: false,
        blocked_reason: None,
        blocked_reason_token: None,
        target: OpTarget {
            path: path.to_string(),
        },
        kind: OpKind::TomlTransform {
            rule_id: rule_id.to_string(),
            args: None,
        },
        rationale: Rationale {
            fix_key: fix_key.to_string(),
            description: None,
            findings: findings
                .into_iter()
                .map(|(source, check_id, code)| FindingRef {
                    source,
                    check_id,
                    code,
                    path: None,
                    line: None,
                    fingerprint: None,
                })
                .collect(),
        },
        params_required: vec![],
        preview: None,
    }
}

// =============================================================================
// ALLOW/DENY POLICY TESTS
// =============================================================================

mod allow_deny_tests {
    use super::*;

    #[test]
    fn empty_allow_and_empty_deny_allows_all() {
        let mut ops = vec![
            make_plan_op(
                "a/Cargo.toml",
                "set_package_rust_version",
                "cargo.normalize_rust_version",
            ),
            make_plan_op(
                "b/Cargo.toml",
                "set_package_license",
                "cargo.normalize_license",
            ),
        ];

        apply_allow_deny(&[], &[], &mut ops);

        assert!(ops.iter().all(|op| !op.blocked));
    }

    #[test]
    fn deny_exact_match_blocks_op() {
        let mut ops = vec![make_plan_op(
            "Cargo.toml",
            "set_package_rust_version",
            "cargo.normalize_rust_version",
        )];

        apply_allow_deny(&[], &["cargo.normalize_rust_version".to_string()], &mut ops);

        assert!(ops[0].blocked);
        assert_eq!(
            ops[0].blocked_reason_token,
            Some(blocked_tokens::DENYLIST.to_string())
        );
    }

    #[test]
    fn deny_wildcard_blocks_matching_ops() {
        let mut ops = vec![
            make_plan_op("a/Cargo.toml", "rule", "cargo.normalize_rust_version"),
            make_plan_op("b/Cargo.toml", "rule", "cargo.normalize_license"),
            make_plan_op("c/Cargo.toml", "rule", "clippy.lint"),
        ];

        apply_allow_deny(&[], &["cargo.*".to_string()], &mut ops);

        assert!(ops[0].blocked); // cargo.normalize_rust_version
        assert!(ops[1].blocked); // cargo.normalize_license
        assert!(!ops[2].blocked); // clippy.lint
    }

    #[test]
    fn allow_exact_match_allows_op() {
        let mut ops = vec![make_plan_op(
            "Cargo.toml",
            "set_package_rust_version",
            "cargo.normalize_rust_version",
        )];

        apply_allow_deny(&["cargo.normalize_rust_version".to_string()], &[], &mut ops);

        assert!(!ops[0].blocked);
    }

    #[test]
    fn allow_wildcard_allows_matching_ops() {
        let mut ops = vec![
            make_plan_op("a/Cargo.toml", "rule", "cargo.normalize_rust_version"),
            make_plan_op("b/Cargo.toml", "rule", "cargo.normalize_license"),
            make_plan_op("c/Cargo.toml", "rule", "clippy.lint"),
        ];

        apply_allow_deny(&["cargo.*".to_string()], &[], &mut ops);

        assert!(!ops[0].blocked); // cargo.normalize_rust_version
        assert!(!ops[1].blocked); // cargo.normalize_license
        assert!(ops[2].blocked); // clippy.lint - not in allowlist
        assert_eq!(
            ops[2].blocked_reason_token,
            Some(blocked_tokens::ALLOWLIST_MISSING.to_string())
        );
    }

    #[test]
    fn deny_takes_precedence_over_allow() {
        // When both allow and deny match, deny should win
        let mut ops = vec![make_plan_op(
            "Cargo.toml",
            "rule",
            "cargo.normalize_rust_version",
        )];

        apply_allow_deny(
            &["cargo.*".to_string()],
            &["cargo.normalize_rust_version".to_string()],
            &mut ops,
        );

        assert!(ops[0].blocked);
        assert_eq!(
            ops[0].blocked_reason_token,
            Some(blocked_tokens::DENYLIST.to_string())
        );
    }

    #[test]
    fn already_blocked_ops_remain_blocked() {
        let mut ops = vec![make_plan_op(
            "Cargo.toml",
            "rule",
            "cargo.normalize_rust_version",
        )];
        ops[0].blocked = true;
        ops[0].blocked_reason = Some("pre-existing block".to_string());
        ops[0].blocked_reason_token = Some("PRE_EXISTING".to_string());

        // Try to deny - should not change existing block
        apply_allow_deny(&[], &["cargo.*".to_string()], &mut ops);

        assert!(ops[0].blocked);
        assert_eq!(
            ops[0].blocked_reason,
            Some("pre-existing block".to_string())
        );
    }

    #[test]
    fn multiple_deny_patterns() {
        let mut ops = vec![
            make_plan_op("a.toml", "rule", "cargo.normalize_rust_version"),
            make_plan_op("b.toml", "rule", "clippy.lint"),
            make_plan_op("c.toml", "rule", "rustfmt.format"),
        ];

        apply_allow_deny(
            &[],
            &["cargo.*".to_string(), "clippy.*".to_string()],
            &mut ops,
        );

        assert!(ops[0].blocked); // cargo.*
        assert!(ops[1].blocked); // clippy.*
        assert!(!ops[2].blocked); // rustfmt.format
    }

    #[test]
    fn multiple_allow_patterns() {
        let mut ops = vec![
            make_plan_op("a.toml", "rule", "cargo.normalize_rust_version"),
            make_plan_op("b.toml", "rule", "clippy.lint"),
            make_plan_op("c.toml", "rule", "rustfmt.format"),
        ];

        apply_allow_deny(
            &["cargo.*".to_string(), "clippy.*".to_string()],
            &[],
            &mut ops,
        );

        assert!(!ops[0].blocked); // cargo.*
        assert!(!ops[1].blocked); // clippy.*
        assert!(ops[2].blocked); // rustfmt.format not in allowlist
    }

    #[test]
    fn findings_based_fix_key_matching() {
        // Ops with findings use source/check_id/code format for matching
        let mut ops = vec![make_plan_op_with_findings(
            "Cargo.toml",
            "rule",
            "cargo.deny",
            vec![(
                "cargo-deny".to_string(),
                Some("licenses".to_string()),
                "missing".to_string(),
            )],
        )];

        // Should match the constructed finding key format
        apply_allow_deny(&[], &["cargo-deny/licenses/missing".to_string()], &mut ops);

        assert!(ops[0].blocked);
    }
}

// =============================================================================
// CAPS HANDLING TESTS
// =============================================================================

mod caps_tests {
    use super::*;

    #[test]
    fn max_ops_cap_blocks_all_when_exceeded() {
        let mut ops = vec![
            make_plan_op("a.toml", "rule", "fix.a"),
            make_plan_op("b.toml", "rule", "fix.b"),
            make_plan_op("c.toml", "rule", "fix.c"),
        ];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: Some(2),
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        enforce_caps(&cfg, &mut ops).unwrap();

        // All ops should be blocked because 3 > 2
        assert!(ops.iter().all(|op| op.blocked));
        assert!(
            ops.iter()
                .all(|op| op.blocked_reason_token.as_deref() == Some(blocked_tokens::MAX_OPS))
        );
    }

    #[test]
    fn max_ops_cap_does_not_block_when_within_limit() {
        let mut ops = vec![
            make_plan_op("a.toml", "rule", "fix.a"),
            make_plan_op("b.toml", "rule", "fix.b"),
        ];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: Some(5),
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        enforce_caps(&cfg, &mut ops).unwrap();

        // No ops should be blocked because 2 <= 5
        assert!(ops.iter().all(|op| !op.blocked));
    }

    #[test]
    fn max_ops_exact_limit_not_blocked() {
        let mut ops = vec![
            make_plan_op("a.toml", "rule", "fix.a"),
            make_plan_op("b.toml", "rule", "fix.b"),
        ];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: Some(2),
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        enforce_caps(&cfg, &mut ops).unwrap();

        // Exactly at limit should not be blocked
        assert!(ops.iter().all(|op| !op.blocked));
    }

    #[test]
    fn max_files_cap_blocks_all_when_exceeded() {
        let mut ops = vec![
            make_plan_op("a.toml", "rule", "fix.a"),
            make_plan_op("b.toml", "rule", "fix.b"),
            make_plan_op("c.toml", "rule", "fix.c"),
        ];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: None,
            max_files: Some(2),
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        enforce_caps(&cfg, &mut ops).unwrap();

        // All ops should be blocked because 3 files > 2
        assert!(ops.iter().all(|op| op.blocked));
        assert!(
            ops.iter()
                .all(|op| op.blocked_reason_token.as_deref() == Some(blocked_tokens::MAX_FILES))
        );
    }

    #[test]
    fn max_files_counts_unique_paths() {
        // Multiple ops targeting same file should count as one file
        let mut ops = vec![
            make_plan_op("a.toml", "rule1", "fix.a"),
            make_plan_op("a.toml", "rule2", "fix.b"),
            make_plan_op("b.toml", "rule", "fix.c"),
        ];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: None,
            max_files: Some(2),
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        enforce_caps(&cfg, &mut ops).unwrap();

        // 2 unique files, limit is 2 - should not block
        assert!(ops.iter().all(|op| !op.blocked));
    }

    #[test]
    fn no_caps_does_not_block() {
        let mut ops = vec![
            make_plan_op("a.toml", "rule", "fix.a"),
            make_plan_op("b.toml", "rule", "fix.b"),
            make_plan_op("c.toml", "rule", "fix.c"),
        ];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: None,
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        enforce_caps(&cfg, &mut ops).unwrap();

        assert!(ops.iter().all(|op| !op.blocked));
    }

    #[test]
    fn max_ops_takes_precedence_over_max_files() {
        // When max_ops is exceeded, it blocks first (max_files not checked)
        let mut ops = vec![
            make_plan_op("a.toml", "rule", "fix.a"),
            make_plan_op("b.toml", "rule", "fix.b"),
        ];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: Some(1),    // Will be exceeded
            max_files: Some(10), // Would not be exceeded
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        enforce_caps(&cfg, &mut ops).unwrap();

        assert!(ops.iter().all(|op| op.blocked));
        assert!(
            ops.iter()
                .all(|op| op.blocked_reason_token.as_deref() == Some(blocked_tokens::MAX_OPS))
        );
    }

    #[test]
    fn empty_ops_list_with_caps() {
        let ops: Vec<PlanOp> = vec![];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: Some(0),
            max_files: Some(0),
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        let mut ops_mut = ops;
        enforce_caps(&cfg, &mut ops_mut).unwrap();

        // Empty list should not panic or block anything
        assert!(ops_mut.is_empty());
    }
}

// =============================================================================
// PARAMS HANDLING TESTS
// =============================================================================

mod params_tests {
    use super::*;

    #[test]
    fn no_params_required_does_not_block() {
        let mut ops = vec![make_plan_op("Cargo.toml", "rule", "fix.key")];

        apply_params(&HashMap::new(), &mut ops);

        assert!(!ops[0].blocked);
    }

    #[test]
    fn all_params_provided_fills_args() {
        let mut ops = vec![PlanOp {
            id: String::new(),
            safety: SafetyClass::Unsafe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: OpKind::TomlTransform {
                rule_id: "set_package_license".into(),
                args: None,
            },
            rationale: Rationale {
                fix_key: "test.fix".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec!["license".to_string()],
            preview: None,
        }];

        let params = HashMap::from([("license".to_string(), "MIT".to_string())]);

        apply_params(&params, &mut ops);

        assert!(!ops[0].blocked);
        assert!(ops[0].params_required.is_empty());

        // Verify the param was filled into args
        if let OpKind::TomlTransform { args, .. } = &ops[0].kind {
            assert!(args.is_some());
            let args = args.as_ref().unwrap();
            assert_eq!(args["license"], serde_json::json!("MIT"));
        } else {
            panic!("Expected TomlTransform");
        }
    }

    #[test]
    fn missing_params_blocks_op() {
        let mut ops = vec![PlanOp {
            id: String::new(),
            safety: SafetyClass::Unsafe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: OpKind::TomlTransform {
                rule_id: "test".into(),
                args: None,
            },
            rationale: Rationale {
                fix_key: "test.fix".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec!["version".to_string(), "author".to_string()],
            preview: None,
        }];

        // Only provide one of two required params
        let params = HashMap::from([("version".to_string(), "1.0.0".to_string())]);

        apply_params(&params, &mut ops);

        assert!(ops[0].blocked);
        assert_eq!(
            ops[0].blocked_reason_token,
            Some(blocked_tokens::MISSING_PARAMS.to_string())
        );
        assert!(ops[0].blocked_reason.as_ref().unwrap().contains("author"));
    }

    #[test]
    fn multiple_ops_with_different_params() {
        let mut ops = vec![
            PlanOp {
                id: String::new(),
                safety: SafetyClass::Unsafe,
                blocked: false,
                blocked_reason: None,
                blocked_reason_token: None,
                target: OpTarget {
                    path: "a.toml".into(),
                },
                kind: OpKind::TomlTransform {
                    rule_id: "set_package_rust_version".into(),
                    args: None,
                },
                rationale: Rationale {
                    fix_key: "fix.a".into(),
                    description: None,
                    findings: vec![],
                },
                params_required: vec!["rust_version".to_string()],
                preview: None,
            },
            PlanOp {
                id: String::new(),
                safety: SafetyClass::Safe,
                blocked: false,
                blocked_reason: None,
                blocked_reason_token: None,
                target: OpTarget {
                    path: "b.toml".into(),
                },
                kind: OpKind::TomlTransform {
                    rule_id: "rule".into(),
                    args: None,
                },
                rationale: Rationale {
                    fix_key: "fix.b".into(),
                    description: None,
                    findings: vec![],
                },
                params_required: vec![], // No params required
                preview: None,
            },
        ];

        let params = HashMap::from([("rust_version".to_string(), "1.70.0".to_string())]);

        apply_params(&params, &mut ops);

        // First op should have params filled
        assert!(!ops[0].blocked);
        assert!(ops[0].params_required.is_empty());

        // Second op should be unchanged
        assert!(!ops[1].blocked);
    }
}

// =============================================================================
// GLOB MATCHING TESTS
// =============================================================================

mod glob_match_tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert!(glob_match("foo", "foo"));
        assert!(glob_match(
            "cargo.normalize_rust_version",
            "cargo.normalize_rust_version"
        ));
        assert!(!glob_match("foo", "bar"));
        assert!(!glob_match("foo", "foobar"));
    }

    #[test]
    fn star_matches_anything() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "cargo.normalize_rust_version"));
    }

    #[test]
    fn star_at_end() {
        assert!(glob_match("cargo.*", "cargo.normalize_rust_version"));
        assert!(glob_match("cargo.*", "cargo.normalize_license"));
        assert!(glob_match("cargo.*", "cargo."));
        assert!(!glob_match("cargo.*", "clippy.lint"));
    }

    #[test]
    fn star_at_start() {
        assert!(glob_match("*.license", "cargo.license"));
        assert!(glob_match("*.license", "clippy.license"));
        assert!(!glob_match("*.license", "cargo.lint"));
    }

    #[test]
    fn star_in_middle() {
        assert!(glob_match("cargo.*.fix", "cargo.something.fix"));
        assert!(glob_match("cargo.*.fix", "cargo..fix"));
        assert!(!glob_match("cargo.*.fix", "cargo.fix"));
    }

    #[test]
    fn question_single_char() {
        assert!(glob_match("foo?", "foo1"));
        assert!(glob_match("foo?", "foox"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("foo?", "foo"));
        assert!(!glob_match("foo?", "foo12"));
    }

    #[test]
    fn multiple_question_marks() {
        assert!(glob_match("???", "abc"));
        assert!(glob_match("a??d", "abcd"));
        assert!(!glob_match("???", "ab"));
        assert!(!glob_match("???", "abcd"));
    }

    #[test]
    fn combined_wildcards() {
        assert!(glob_match("foo*?", "foobar"));
        assert!(glob_match("foo*?", "foox"));
        assert!(!glob_match("foo*?", "foo")); // * can match empty, but ? needs one char
        assert!(glob_match("?*?", "ab"));
        assert!(glob_match("?*?", "abc"));
    }

    #[test]
    fn empty_pattern_and_text() {
        assert!(glob_match("", ""));
        assert!(!glob_match("", "a"));
        assert!(!glob_match("a", ""));
    }

    #[test]
    fn complex_policy_patterns() {
        // Realistic policy patterns
        assert!(glob_match("cargo-deny/*", "cargo-deny/licenses.missing"));
        assert!(glob_match("cargo-deny/*", "cargo-deny/bans.duplicate"));
        assert!(!glob_match("cargo-deny/*", "cargo-allow/licenses.missing"));

        assert!(glob_match("*/licenses/*", "cargo-deny/licenses/missing"));
        assert!(glob_match("clippy.lint_*", "clippy.lint_unused"));
    }

    #[test]
    fn special_regex_chars_treated_literally() {
        // Glob patterns should treat regex special chars as literals
        assert!(glob_match("cargo-deny", "cargo-deny"));
        assert!(glob_match("test.value", "test.value"));
        assert!(glob_match("test[value]", "test[value]"));
        assert!(!glob_match("test.value", "testXvalue"));
    }
}

// =============================================================================
// DETERMINISTIC ID AND SORTING TESTS
// =============================================================================

mod deterministic_tests {
    use super::*;

    #[test]
    fn same_op_produces_same_id() {
        let op = make_plan_op("Cargo.toml", "rule", "fix.key");

        let id1 = deterministic_op_id(&op);
        let id2 = deterministic_op_id(&op);

        assert_eq!(id1, id2);
    }

    #[test]
    fn different_ops_produce_different_ids() {
        let op1 = make_plan_op("a.toml", "rule", "fix.a");
        let op2 = make_plan_op("b.toml", "rule", "fix.b");

        let id1 = deterministic_op_id(&op1);
        let id2 = deterministic_op_id(&op2);

        assert_ne!(id1, id2);
    }

    #[test]
    fn id_is_deterministic_across_calls() {
        let op = make_plan_op(
            "Cargo.toml",
            "ensure_workspace_resolver_v2",
            "cargo.workspace_resolver_v2",
        );

        // Call multiple times - should always produce same ID
        let ids: std::collections::HashSet<_> = (0..10).map(|_| deterministic_op_id(&op)).collect();

        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn sort_key_is_deterministic() {
        let op = make_plan_op("Cargo.toml", "rule", "fix.key");

        let key1 = stable_op_sort_key(&op);
        let key2 = stable_op_sort_key(&op);

        assert_eq!(key1, key2);
    }

    #[test]
    fn sort_key_orders_by_fix_key_then_path() {
        let mut ops = [
            make_plan_op("z.toml", "rule", "fix.b"),
            make_plan_op("a.toml", "rule", "fix.a"),
            make_plan_op("m.toml", "rule", "fix.a"),
        ];

        ops.sort_by_key(stable_op_sort_key);

        // Should be sorted by fix_key first, then path
        assert_eq!(ops[0].rationale.fix_key, "fix.a");
        assert_eq!(ops[0].target.path, "a.toml");
        assert_eq!(ops[1].rationale.fix_key, "fix.a");
        assert_eq!(ops[1].target.path, "m.toml");
        assert_eq!(ops[2].rationale.fix_key, "fix.b");
        assert_eq!(ops[2].target.path, "z.toml");
    }

    #[test]
    fn args_fingerprint_is_consistent() {
        let args1 = Some(serde_json::json!({"a": 1, "b": 2}));
        let args2 = Some(serde_json::json!({"b": 2, "a": 1})); // Same content, different order

        assert_eq!(args_fingerprint(&args1), args_fingerprint(&args2));
    }

    #[test]
    fn args_fingerprint_none_is_consistent() {
        let fp1 = args_fingerprint(&None);
        let fp2 = args_fingerprint(&None);

        assert_eq!(fp1, fp2);
        assert_eq!(fp1, "no_args");
    }

    #[test]
    fn args_fingerprint_different_values_differ() {
        let args1 = Some(serde_json::json!({"a": 1}));
        let args2 = Some(serde_json::json!({"a": 2}));

        assert_ne!(args_fingerprint(&args1), args_fingerprint(&args2));
    }

    #[test]
    fn args_fingerprint_nested_objects() {
        let args1 = Some(serde_json::json!({
            "outer": {
                "inner_a": 1,
                "inner_b": 2
            }
        }));
        let args2 = Some(serde_json::json!({
            "outer": {
                "inner_b": 2,
                "inner_a": 1
            }
        }));

        // Nested object key order shouldn't matter
        assert_eq!(args_fingerprint(&args1), args_fingerprint(&args2));
    }
}

// =============================================================================
// INTEGRATION TESTS - apply_plan_policy
// =============================================================================

mod integration_tests {
    use super::*;

    #[test]
    fn full_policy_pipeline() {
        let mut ops = vec![
            make_plan_op("b.toml", "rule", "cargo.fix_b"),
            make_plan_op("a.toml", "rule", "cargo.fix_a"),
            make_plan_op("c.toml", "rule", "clippy.lint"),
        ];

        let cfg = PlannerConfig {
            allow: vec!["cargo.*".to_string()],
            deny: vec!["cargo.fix_b".to_string()],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: None,
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        apply_plan_policy(&cfg, &mut ops).unwrap();

        // Should be sorted by fix_key
        assert_eq!(ops[0].rationale.fix_key, "cargo.fix_a");
        assert_eq!(ops[1].rationale.fix_key, "cargo.fix_b");
        assert_eq!(ops[2].rationale.fix_key, "clippy.lint");

        // All should have IDs assigned
        assert!(ops.iter().all(|op| !op.id.is_empty()));

        // cargo.fix_a: in allowlist, not in denylist -> allowed
        assert!(!ops[0].blocked);

        // cargo.fix_b: in denylist -> blocked
        assert!(ops[1].blocked);
        assert_eq!(
            ops[1].blocked_reason_token,
            Some(blocked_tokens::DENYLIST.to_string())
        );

        // clippy.lint: not in allowlist -> blocked
        assert!(ops[2].blocked);
        assert_eq!(
            ops[2].blocked_reason_token,
            Some(blocked_tokens::ALLOWLIST_MISSING.to_string())
        );
    }

    #[test]
    fn policy_with_caps_and_params() {
        let mut ops = vec![PlanOp {
            id: String::new(),
            safety: SafetyClass::Unsafe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: OpKind::TomlTransform {
                rule_id: "set_package_license".into(),
                args: None,
            },
            rationale: Rationale {
                fix_key: "cargo.normalize_license".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec!["license".to_string()],
            preview: None,
        }];

        let cfg = PlannerConfig {
            allow: vec!["cargo.*".to_string()],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: Some(1),
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::from([("license".to_string(), "MIT".to_string())]),
        };

        apply_plan_policy(&cfg, &mut ops).unwrap();

        // Params should be filled
        assert!(ops[0].params_required.is_empty());

        // Should have ID
        assert!(!ops[0].id.is_empty());

        // Should not be blocked (within caps, in allowlist, params provided)
        assert!(!ops[0].blocked);
    }

    #[test]
    fn empty_ops_list() {
        let mut ops: Vec<PlanOp> = vec![];

        let cfg = PlannerConfig {
            allow: vec![],
            deny: vec![],
            allow_guarded: false,
            allow_unsafe: false,
            allow_dirty: false,
            max_ops: None,
            max_files: None,
            max_patch_bytes: None,
            params: HashMap::new(),
        };

        apply_plan_policy(&cfg, &mut ops).unwrap();

        assert!(ops.is_empty());
    }
}

// =============================================================================
// OP KIND VARIANTS TESTS
// =============================================================================

mod op_kind_tests {
    use super::*;

    #[test]
    fn toml_set_sort_key() {
        let op = PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: OpKind::TomlSet {
                toml_path: vec!["package".into(), "version".into()],
                value: "1.0.0".into(),
            },
            rationale: Rationale {
                fix_key: "test.fix".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        };

        let key = stable_op_sort_key(&op);
        assert!(key.contains("set|"));
        assert!(key.contains("package.version"));
    }

    #[test]
    fn toml_remove_sort_key() {
        let op = PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: OpKind::TomlRemove {
                toml_path: vec!["dependencies".into(), "unused".into()],
            },
            rationale: Rationale {
                fix_key: "test.remove".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        };

        let key = stable_op_sort_key(&op);
        assert!(key.contains("remove|"));
        assert!(key.contains("dependencies.unused"));
    }

    #[test]
    fn json_set_sort_key() {
        let op = PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "config.json".into(),
            },
            kind: OpKind::JsonSet {
                json_path: vec!["settings".into(), "debug".into()],
                value: serde_json::json!(true),
            },
            rationale: Rationale {
                fix_key: "test.json".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        };

        let key = stable_op_sort_key(&op);
        assert!(key.contains("json_set|"));
    }

    #[test]
    fn text_replace_anchored_sort_key() {
        let op = PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "src/lib.rs".into(),
            },
            kind: OpKind::TextReplaceAnchored {
                find: "old".into(),
                replace: "new".into(),
                anchor_before: vec!["fn ".into()],
                anchor_after: vec![" {".into()],
                max_replacements: Some(1),
            },
            rationale: Rationale {
                fix_key: "test.replace".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        };

        let key = stable_op_sort_key(&op);
        assert!(key.contains("text_replace_anchored|"));
        assert!(key.contains("old"));
        assert!(key.contains("new"));
    }

    #[test]
    fn deterministic_id_different_op_kinds() {
        let op1 = PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: OpKind::TomlSet {
                toml_path: vec!["version".into()],
                value: "1.0".into(),
            },
            rationale: Rationale {
                fix_key: "test".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        };

        let op2 = PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".into(),
            },
            kind: OpKind::TomlRemove {
                toml_path: vec!["version".into()],
            },
            rationale: Rationale {
                fix_key: "test".into(),
                description: None,
                findings: vec![],
            },
            params_required: vec![],
            preview: None,
        };

        // Different op kinds should produce different IDs
        assert_ne!(deterministic_op_id(&op1), deterministic_op_id(&op2));
    }
}
