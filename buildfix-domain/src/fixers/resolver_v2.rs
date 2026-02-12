use crate::fixers::{Fixer, FixerMeta};
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{PlanOp, Rationale};
use camino::Utf8PathBuf;
use toml_edit::DocumentMut;

pub struct ResolverV2Fixer;

impl ResolverV2Fixer {
    const FIX_ID: &'static str = "cargo.workspace_resolver_v2";
    const DESCRIPTION: &'static str =
        "Sets [workspace].resolver = \"2\" for correct feature unification";
    const SENSORS: &'static [&'static str] = &["builddiag", "cargo"];
    const CHECK_IDS: &'static [&'static str] =
        &["workspace.resolver_v2", "cargo.workspace.resolver_v2"];

    fn needs_fix(repo: &dyn RepoView, manifest: &Utf8PathBuf) -> bool {
        let contents = match repo.read_to_string(manifest) {
            Ok(c) => c,
            Err(_) => return false,
        };

        let doc = match contents.parse::<DocumentMut>() {
            Ok(d) => d,
            Err(_) => return false,
        };

        let ws = match doc.get("workspace").and_then(|i| i.as_table()) {
            Some(t) => t,
            None => return false, // Not a workspace; resolver-v2 is inapplicable.
        };

        let resolver = ws
            .get("resolver")
            .and_then(|i| i.as_value())
            .and_then(|v| v.as_str());

        resolver != Some("2")
    }
}

impl Fixer for ResolverV2Fixer {
    fn meta(&self) -> FixerMeta {
        FixerMeta {
            fix_key: Self::FIX_ID,
            description: Self::DESCRIPTION,
            safety: SafetyClass::Safe,
            consumes_sensors: Self::SENSORS,
            consumes_check_ids: Self::CHECK_IDS,
        }
    }

    fn plan(
        &self,
        _ctx: &crate::planner::PlanContext,
        repo: &dyn RepoView,
        receipts: &ReceiptSet,
    ) -> anyhow::Result<Vec<PlanOp>> {
        let triggers = receipts.matching_findings(Self::SENSORS, Self::CHECK_IDS, &[]);
        if triggers.is_empty() {
            return Ok(vec![]);
        }

        let manifest: Utf8PathBuf = "Cargo.toml".into();
        if !Self::needs_fix(repo, &manifest) {
            return Ok(vec![]);
        }

        let fix_key = triggers
            .first()
            .map(fix_key_for)
            .unwrap_or_else(|| "unknown/-/-".to_string());

        Ok(vec![PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: manifest.to_string(),
            },
            kind: OpKind::TomlTransform {
                rule_id: "ensure_workspace_resolver_v2".to_string(),
                args: None,
            },
            rationale: Rationale {
                fix_key,
                description: Some(Self::DESCRIPTION.to_string()),
                findings: triggers,
            },
            params_required: vec![],
            preview: None,
        }])
    }
}

fn fix_key_for(f: &buildfix_types::plan::FindingRef) -> String {
    let check = f.check_id.clone().unwrap_or_else(|| "-".to_string());
    format!("{}/{}/{}", f.source, check, f.code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{PlanContext, PlannerConfig, ReceiptSet};
    use crate::ports::RepoView;
    use buildfix_receipts::LoadedReceipt;
    use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, RunInfo, ToolInfo, Verdict};
    use camino::{Utf8Path, Utf8PathBuf};
    use std::collections::HashMap;

    struct TestRepo {
        root: Utf8PathBuf,
        files: HashMap<String, String>,
    }

    impl TestRepo {
        fn new(files: &[(&str, &str)]) -> Self {
            let mut map = HashMap::new();
            for (path, contents) in files {
                map.insert(path.to_string(), contents.to_string());
            }
            Self {
                root: Utf8PathBuf::from("."),
                files: map,
            }
        }

        fn key_for(&self, rel: &Utf8Path) -> String {
            if rel.is_absolute() {
                rel.strip_prefix(&self.root).unwrap_or(rel).to_string()
            } else {
                rel.to_string()
            }
        }
    }

    impl RepoView for TestRepo {
        fn root(&self) -> &Utf8Path {
            &self.root
        }

        fn read_to_string(&self, rel: &Utf8Path) -> anyhow::Result<String> {
            let key = self.key_for(rel);
            self.files
                .get(&key)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing {}", key))
        }

        fn exists(&self, rel: &Utf8Path) -> bool {
            let key = self.key_for(rel);
            self.files.contains_key(&key)
        }
    }

    fn receipt_set() -> ReceiptSet {
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: ToolInfo {
                name: "cargo".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("workspace.resolver_v2".to_string()),
                code: Some("RESOLVER".to_string()),
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
            path: Utf8PathBuf::from("artifacts/cargo/report.json"),
            sensor_id: "cargo".to_string(),
            receipt: Ok(receipt),
        }];
        ReceiptSet::from_loaded(&loaded)
    }

    #[test]
    fn needs_fix_handles_workspace_resolver() {
        let repo = TestRepo::new(&[("Cargo.toml", r#"[package]\nname = "demo""#)]);
        assert!(!ResolverV2Fixer::needs_fix(
            &repo,
            &Utf8PathBuf::from("Cargo.toml")
        ));

        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "2"
            "#,
        )]);
        assert!(!ResolverV2Fixer::needs_fix(
            &repo,
            &Utf8PathBuf::from("Cargo.toml")
        ));

        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "1"
            "#,
        )]);
        assert!(ResolverV2Fixer::needs_fix(
            &repo,
            &Utf8PathBuf::from("Cargo.toml")
        ));
    }

    #[test]
    fn plan_emits_fix_when_triggered() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [workspace]
                resolver = "1"
            "#,
        )]);
        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };
        let fixes = ResolverV2Fixer
            .plan(&ctx, &repo, &receipt_set())
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        let op = &fixes[0];
        assert_eq!(op.safety, SafetyClass::Safe);
        assert!(matches!(op.kind, OpKind::TomlTransform { .. }));
        if let OpKind::TomlTransform { rule_id, args } = &op.kind {
            assert_eq!(rule_id, "ensure_workspace_resolver_v2");
            assert!(args.is_none());
        }
    }

    #[test]
    fn needs_fix_returns_false_on_missing_or_invalid_manifest() {
        let repo_missing = TestRepo::new(&[]);
        assert!(!ResolverV2Fixer::needs_fix(
            &repo_missing,
            &Utf8PathBuf::from("Cargo.toml")
        ));

        let repo_invalid = TestRepo::new(&[("Cargo.toml", "not toml = [")]);
        assert!(!ResolverV2Fixer::needs_fix(
            &repo_invalid,
            &Utf8PathBuf::from("Cargo.toml")
        ));

        let repo_no_workspace = TestRepo::new(&[("Cargo.toml", "[package]\nname = \"demo\"")]);
        assert!(!ResolverV2Fixer::needs_fix(
            &repo_no_workspace,
            &Utf8PathBuf::from("Cargo.toml")
        ));
    }

    #[test]
    fn test_repo_helpers_handle_absolute_paths() {
        let root = Utf8PathBuf::from_path_buf(std::env::current_dir().expect("cwd")).expect("utf8");
        let mut files = HashMap::new();
        files.insert(
            "Cargo.toml".to_string(),
            "[workspace]\nresolver = \"1\"\n".to_string(),
        );
        let repo = TestRepo {
            root: root.clone(),
            files,
        };
        let abs = root.join("Cargo.toml");
        assert!(repo.exists(&abs));
        assert!(repo.read_to_string(&abs).unwrap().contains("resolver"));
        assert_eq!(repo.root(), root.as_path());
    }

    #[test]
    fn fix_key_for_handles_missing_check_id() {
        let f = buildfix_types::plan::FindingRef {
            source: "builddiag".to_string(),
            check_id: None,
            code: "X".to_string(),
            path: None,
            line: None,
            fingerprint: None,
        };
        assert_eq!(super::fix_key_for(&f), "builddiag/-/X");
    }
}
