use crate::fixers::{Fixer, FixerMeta};
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{PlanOp, Rationale};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::DocumentMut;

pub struct MsrvNormalizeFixer;

impl MsrvNormalizeFixer {
    const FIX_ID: &'static str = "cargo.normalize_rust_version";
    const DESCRIPTION: &'static str =
        "Normalizes per-crate MSRV to workspace canonical rust-version";
    const SENSORS: &'static [&'static str] = &["builddiag", "cargo"];
    const CHECK_IDS: &'static [&'static str] = &[
        "rust.msrv_consistent",
        "cargo.msrv_consistent",
        "msrv.consistent",
    ];

    fn canonical_rust_version(repo: &dyn RepoView) -> Option<String> {
        let contents = repo.read_to_string(Utf8Path::new("Cargo.toml")).ok()?;
        let doc = contents.parse::<DocumentMut>().ok()?;

        // Preferred: [workspace.package].rust-version
        if let Some(ws) = doc.get("workspace").and_then(|i| i.as_table())
            && let Some(pkg) = ws.get("package").and_then(|i| i.as_table())
            && let Some(v) = pkg
                .get("rust-version")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
        {
            return Some(v.to_string());
        }

        // Fallback: [package].rust-version
        if let Some(pkg) = doc.get("package").and_then(|i| i.as_table())
            && let Some(v) = pkg
                .get("rust-version")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
        {
            return Some(v.to_string());
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

    fn needs_change(contents: &str, rust_version: &str) -> bool {
        let Ok(doc) = contents.parse::<DocumentMut>() else {
            return true;
        };
        let Some(pkg) = doc.get("package").and_then(|i| i.as_table()) else {
            return true;
        };

        let current = pkg
            .get("rust-version")
            .and_then(|i| i.as_value())
            .and_then(|v| v.as_str());

        current != Some(rust_version)
    }
}

impl Fixer for MsrvNormalizeFixer {
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

        let rust_version = Self::canonical_rust_version(repo);

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
            if let Some(rv) = &rust_version
                && !Self::needs_change(&contents, rv)
            {
                continue;
            }

            let (safety, params_required, rust_version_value) = match &rust_version {
                Some(rv) => (
                    SafetyClass::Guarded,
                    vec![],
                    serde_json::Value::String(rv.clone()),
                ),
                None => (
                    SafetyClass::Unsafe,
                    vec!["rust_version".to_string()],
                    serde_json::Value::Null,
                ),
            };

            let mut args = serde_json::Map::new();
            args.insert("rust_version".to_string(), rust_version_value);

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
                    rule_id: "set_package_rust_version".to_string(),
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
                check_id: Some("msrv.consistent".to_string()),
                code: Some("MSRV".to_string()),
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
    fn canonical_rust_version_prefers_workspace_package() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [workspace.package]
                rust-version = "1.70"

                [package]
                rust-version = "1.60"
            "#,
        )]);
        let version = MsrvNormalizeFixer::canonical_rust_version(&repo);
        assert_eq!(version.as_deref(), Some("1.70"));
    }

    #[test]
    fn canonical_rust_version_falls_back_to_package() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [package]
                rust-version = "1.60"
            "#,
        )]);
        let version = MsrvNormalizeFixer::canonical_rust_version(&repo);
        assert_eq!(version.as_deref(), Some("1.60"));
    }

    #[test]
    fn needs_change_detects_mismatch() {
        let manifest = r#"
            [package]
            rust-version = "1.60"
        "#;
        assert!(MsrvNormalizeFixer::needs_change("not toml", "1.70"));
        assert!(MsrvNormalizeFixer::needs_change("[workspace]", "1.70"));
        assert!(MsrvNormalizeFixer::needs_change(manifest, "1.70"));
        assert!(!MsrvNormalizeFixer::needs_change(manifest, "1.60"));
    }

    #[test]
    fn plan_emits_guarded_fix_with_canonical_version() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.package]
                    rust-version = "1.70"
                "#,
            ),
            (
                "crates/a/Cargo.toml",
                r#"
                    [package]
                    name = "a"
                    rust-version = "1.60"
                "#,
            ),
        ]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let receipt_set = receipt_set_for("crates/a/Cargo.toml");
        let fixes = MsrvNormalizeFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        let op = &fixes[0];
        assert_eq!(op.safety, SafetyClass::Guarded);
        assert!(op.params_required.is_empty());
        match &op.kind {
            OpKind::TomlTransform { rule_id, args } => {
                assert_eq!(rule_id, "set_package_rust_version");
                assert_eq!(args.as_ref().unwrap()["rust_version"], "1.70");
            }
            _ => panic!("expected toml transform"),
        }
    }

    #[test]
    fn plan_emits_unsafe_fix_without_canonical_version() {
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
        let fixes = MsrvNormalizeFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        let op = &fixes[0];
        assert_eq!(op.safety, SafetyClass::Unsafe);
        assert_eq!(op.params_required, vec!["rust_version".to_string()]);
    }
}
