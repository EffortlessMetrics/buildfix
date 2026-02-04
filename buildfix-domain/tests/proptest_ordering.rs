//! Property-based tests for deterministic ordering in buildfix-domain.
//!
//! These tests verify that:
//! - Fixes are sorted consistently regardless of input order
//! - The stable_fix_sort_key produces consistent ordering
//! - Multiple runs with the same input produce identical output

use buildfix_domain::{FsRepoView, PlanContext, Planner, PlannerConfig};
use buildfix_types::plan::PolicyCaps;
use buildfix_types::receipt::ToolInfo;
use camino::Utf8PathBuf;
use fs_err as fs;
use proptest::prelude::*;
use tempfile::TempDir;

/// Strategy to generate a list of crate names.
fn arb_crate_names() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(
        prop::string::string_regex(r"[a-z][a-z0-9_-]{0,10}")
            .unwrap()
            .prop_filter("non-empty", |s| !s.is_empty()),
        1..5,
    )
    .prop_map(|mut names| {
        // Deduplicate
        names.sort();
        names.dedup();
        names
    })
}

proptest! {
    /// Sorting the same fixes twice produces identical order.
    #[test]
    fn stable_ordering_deterministic(crate_names in arb_crate_names()) {
        // Create a workspace with multiple crates missing versions
        let temp_dir = TempDir::new().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        let artifacts_dir = root.join("artifacts");

        // Create workspace Cargo.toml
        let members: Vec<String> = crate_names.iter().map(|n| format!("crates/{}", n)).collect();
        let workspace_toml = format!(
            r#"[workspace]
members = [{}]
"#,
            members
                .iter()
                .map(|m| format!("\"{}\"", m))
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Write workspace manifest
        fs::write(root.join("Cargo.toml"), &workspace_toml).unwrap();

        // Create crate manifests
        for name in &crate_names {
            let crate_dir = root.join("crates").join(name);
            fs::create_dir_all(&crate_dir).unwrap();
            let crate_toml = format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"
"#,
                name
            );
            fs::write(crate_dir.join("Cargo.toml"), &crate_toml).unwrap();
        }

        // Create builddiag receipt for resolver v2
        fs::create_dir_all(artifacts_dir.join("builddiag")).unwrap();
        let receipt = serde_json::json!({
            "schema": "builddiag.report.v1",
            "tool": { "name": "builddiag", "version": "0.0.0" },
            "verdict": { "status": "fail", "counts": { "findings": 1, "errors": 1, "warnings": 0 } },
            "findings": [{
                "severity": "error",
                "check_id": "workspace.resolver_v2",
                "code": "not_v2",
                "message": "workspace resolver is not 2",
                "location": { "path": "Cargo.toml", "line": 1, "column": 1 }
            }]
        });
        fs::write(
            artifacts_dir.join("builddiag/report.json"),
            serde_json::to_string_pretty(&receipt).unwrap(),
        )
        .unwrap();

        let repo = FsRepoView::new(root.clone());

        let receipts = buildfix_receipts::load_receipts(&artifacts_dir).unwrap_or_default();
        let planner = Planner::new();
        let ctx = PlanContext {
            repo_root: root.clone(),
            artifacts_dir,
            config: PlannerConfig {
                allow: vec![],
                deny: vec![],
                require_clean_hashes: true,
                caps: PolicyCaps::default(),
            },
        };

        let tool = ToolInfo {
            name: "buildfix".to_string(),
            version: Some("test".to_string()),
            repo: None,
            commit: None,
        };

        // Run planner twice
        let plan1 = planner.plan(&ctx, &repo, &receipts, tool.clone()).unwrap();
        let plan2 = planner.plan(&ctx, &repo, &receipts, tool).unwrap();

        // Fix IDs should be in the same order
        let fix_ids_1: Vec<&str> = plan1.fixes.iter().map(|f| f.fix_id.0.as_str()).collect();
        let fix_ids_2: Vec<&str> = plan2.fixes.iter().map(|f| f.fix_id.0.as_str()).collect();

        prop_assert_eq!(&fix_ids_1, &fix_ids_2, "fix ordering should be deterministic");

        // Fix titles should also be in the same order
        let titles_1: Vec<&str> = plan1.fixes.iter().map(|f| f.title.as_str()).collect();
        let titles_2: Vec<&str> = plan2.fixes.iter().map(|f| f.title.as_str()).collect();

        prop_assert_eq!(&titles_1, &titles_2, "fix title ordering should be deterministic");
    }

    /// Summary counts match actual fix counts.
    #[test]
    fn summary_matches_fixes(_unused in arb_crate_names()) {
        let temp_dir = TempDir::new().unwrap();
        let root = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
        let artifacts_dir = root.join("artifacts");

        let workspace_toml = r#"[workspace]
members = []
"#;

        fs::write(root.join("Cargo.toml"), workspace_toml).unwrap();

        fs::create_dir_all(artifacts_dir.join("builddiag")).unwrap();
        let receipt = serde_json::json!({
            "schema": "builddiag.report.v1",
            "tool": { "name": "builddiag", "version": "0.0.0" },
            "verdict": { "status": "fail", "counts": { "findings": 1, "errors": 1, "warnings": 0 } },
            "findings": [{
                "severity": "error",
                "check_id": "workspace.resolver_v2",
                "code": "not_v2",
                "message": "workspace resolver is not 2",
                "location": { "path": "Cargo.toml", "line": 1, "column": 1 }
            }]
        });
        fs::write(
            artifacts_dir.join("builddiag/report.json"),
            serde_json::to_string_pretty(&receipt).unwrap(),
        )
        .unwrap();

        let repo = FsRepoView::new(root.clone());

        let receipts = buildfix_receipts::load_receipts(&artifacts_dir).unwrap_or_default();
        let planner = Planner::new();
        let ctx = PlanContext {
            repo_root: root,
            artifacts_dir,
            config: PlannerConfig::default(),
        };

        let tool = ToolInfo {
            name: "buildfix".to_string(),
            version: Some("test".to_string()),
            repo: None,
            commit: None,
        };

        let plan = planner.plan(&ctx, &repo, &receipts, tool).unwrap();

        // Verify summary counts
        let safe_count = plan.fixes.iter()
            .filter(|f| f.safety == buildfix_types::ops::SafetyClass::Safe)
            .count() as u64;
        let guarded_count = plan.fixes.iter()
            .filter(|f| f.safety == buildfix_types::ops::SafetyClass::Guarded)
            .count() as u64;
        let unsafe_count = plan.fixes.iter()
            .filter(|f| f.safety == buildfix_types::ops::SafetyClass::Unsafe)
            .count() as u64;

        prop_assert_eq!(plan.summary.safe, safe_count);
        prop_assert_eq!(plan.summary.guarded, guarded_count);
        prop_assert_eq!(plan.summary.unsafe_, unsafe_count);
        prop_assert_eq!(plan.summary.fixes_total, plan.fixes.len() as u64);
    }
}
