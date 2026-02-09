use crate::fixers::{Fixer, FixerMeta};
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{PlanOp, Rationale};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::DocumentMut;

pub struct EditionUpgradeFixer;

impl EditionUpgradeFixer {
    const FIX_ID: &'static str = "cargo.normalize_edition";
    const DESCRIPTION: &'static str =
        "Normalizes per-crate Rust edition to workspace canonical edition";
    const SENSORS: &'static [&'static str] = &["builddiag", "cargo"];
    const CHECK_IDS: &'static [&'static str] = &[
        "rust.edition_consistent",
        "cargo.edition_consistent",
        "edition.consistent",
    ];

    fn canonical_edition(repo: &dyn RepoView) -> Option<String> {
        let contents = repo.read_to_string(Utf8Path::new("Cargo.toml")).ok()?;
        let doc = contents.parse::<DocumentMut>().ok()?;

        // Preferred: [workspace.package].edition
        if let Some(ws) = doc.get("workspace").and_then(|i| i.as_table())
            && let Some(pkg) = ws.get("package").and_then(|i| i.as_table())
            && let Some(e) = pkg
                .get("edition")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
        {
            return Some(e.to_string());
        }

        // Fallback: [package].edition (for root package in a workspace)
        if let Some(pkg) = doc.get("package").and_then(|i| i.as_table())
            && let Some(e) = pkg
                .get("edition")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
        {
            return Some(e.to_string());
        }

        None
    }

    fn manifest_paths_from_triggers(
        triggers: &[buildfix_types::plan::FindingRef],
    ) -> BTreeSet<Utf8PathBuf> {
        let mut out = BTreeSet::new();
        for t in triggers {
            let Some(path) = &t.path else { continue };
            if path.ends_with("Cargo.toml") {
                out.insert(Utf8PathBuf::from(path.clone()));
            }
        }
        out
    }

    fn needs_change(contents: &str, edition: &str) -> bool {
        let Ok(doc) = contents.parse::<DocumentMut>() else {
            return true;
        };
        let Some(pkg) = doc.get("package").and_then(|i| i.as_table()) else {
            return true;
        };

        let current = pkg
            .get("edition")
            .and_then(|i| i.as_value())
            .and_then(|v| v.as_str());

        current != Some(edition)
    }
}

impl Fixer for EditionUpgradeFixer {
    fn meta(&self) -> FixerMeta {
        FixerMeta {
            fix_key: Self::FIX_ID,
            description: Self::DESCRIPTION,
            safety: SafetyClass::Guarded,
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

        let edition = Self::canonical_edition(repo);

        let mut triggers_by_manifest: BTreeMap<Utf8PathBuf, Vec<buildfix_types::plan::FindingRef>> =
            BTreeMap::new();
        for t in &triggers {
            if let Some(path) = &t.path {
                triggers_by_manifest
                    .entry(Utf8PathBuf::from(path.clone()))
                    .or_default()
                    .push(t.clone());
            }
        }

        let mut fixes = Vec::new();
        for manifest in Self::manifest_paths_from_triggers(&triggers) {
            let contents = match repo.read_to_string(&manifest) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Some(ed) = &edition
                && !Self::needs_change(&contents, ed)
            {
                continue;
            }

            let (safety, params_required, edition_value) = match &edition {
                Some(ed) => (
                    SafetyClass::Guarded,
                    vec![],
                    serde_json::Value::String(ed.clone()),
                ),
                None => (
                    SafetyClass::Unsafe,
                    vec!["edition".to_string()],
                    serde_json::Value::Null,
                ),
            };

            let mut args = serde_json::Map::new();
            args.insert("edition".to_string(), edition_value);

            let findings = triggers_by_manifest
                .get(&manifest)
                .cloned()
                .unwrap_or_else(Vec::new);
            let fix_key = findings
                .first()
                .map(fix_key_for)
                .unwrap_or_else(|| "unknown/-/-".to_string());

            fixes.push(PlanOp {
                id: String::new(),
                safety,
                blocked: false,
                blocked_reason: None,
                blocked_reason_token: None,
                target: OpTarget {
                    path: manifest.to_string(),
                },
                kind: OpKind::TomlTransform {
                    rule_id: "set_package_edition".to_string(),
                    args: Some(serde_json::Value::Object(args)),
                },
                rationale: Rationale {
                    fix_key,
                    description: Some(Self::DESCRIPTION.to_string()),
                    findings,
                },
                params_required,
                preview: None,
            });
        }

        Ok(fixes)
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
                rel.strip_prefix(&self.root)
                    .unwrap_or(rel)
                    .to_string()
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

    fn receipt_set_for(path: &str) -> ReceiptSet {
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
                check_id: Some("edition.consistent".to_string()),
                code: Some("EDITION".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from(path),
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
        ReceiptSet::from_loaded(&loaded)
    }

    #[test]
    fn canonical_edition_prefers_workspace_package() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [workspace.package]
                edition = "2021"

                [package]
                edition = "2018"
            "#,
        )]);
        let edition = EditionUpgradeFixer::canonical_edition(&repo);
        assert_eq!(edition.as_deref(), Some("2021"));
    }

    #[test]
    fn canonical_edition_falls_back_to_package() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [package]
                edition = "2018"
            "#,
        )]);
        let edition = EditionUpgradeFixer::canonical_edition(&repo);
        assert_eq!(edition.as_deref(), Some("2018"));
    }

    #[test]
    fn needs_change_detects_mismatch() {
        let manifest = r#"
            [package]
            edition = "2018"
        "#;
        assert!(EditionUpgradeFixer::needs_change("not toml", "2021"));
        assert!(EditionUpgradeFixer::needs_change("[workspace]", "2021"));
        assert!(EditionUpgradeFixer::needs_change(manifest, "2021"));
        assert!(!EditionUpgradeFixer::needs_change(manifest, "2018"));
    }

    #[test]
    fn plan_emits_guarded_fix_with_canonical_edition() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.package]
                    edition = "2021"
                "#,
            ),
            (
                "crates/a/Cargo.toml",
                r#"
                    [package]
                    name = "a"
                    edition = "2018"
                "#,
            ),
        ]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let receipt_set = receipt_set_for("crates/a/Cargo.toml");
        let fixes = EditionUpgradeFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        let op = &fixes[0];
        assert_eq!(op.safety, SafetyClass::Guarded);
        assert!(op.params_required.is_empty());
        match &op.kind {
            OpKind::TomlTransform { rule_id, args } => {
                assert_eq!(rule_id, "set_package_edition");
                assert_eq!(args.as_ref().unwrap()["edition"], "2021");
            }
            _ => panic!("expected toml transform"),
        }
    }

    #[test]
    fn plan_emits_unsafe_fix_without_canonical_edition() {
        let repo = TestRepo::new(&[(
            "crates/a/Cargo.toml",
            r#"
                [package]
                name = "a"
            "#,
        )]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let receipt_set = receipt_set_for("crates/a/Cargo.toml");
        let fixes = EditionUpgradeFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        let op = &fixes[0];
        assert_eq!(op.safety, SafetyClass::Unsafe);
        assert_eq!(op.params_required, vec!["edition".to_string()]);
    }
}
