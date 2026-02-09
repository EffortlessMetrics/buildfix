use crate::fixers::{Fixer, FixerMeta};
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{PlanOp, Rationale};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::{DocumentMut, Table};

pub struct PathDepVersionFixer;

impl PathDepVersionFixer {
    const FIX_ID: &'static str = "cargo.path_dep_add_version";
    const DESCRIPTION: &'static str = "Adds version field to path dependencies for publishability";
    const SENSORS: &'static [&'static str] = &["depguard"];
    const CHECK_IDS: &'static [&'static str] =
        &["deps.path_requires_version", "cargo.path_requires_version"];

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

    fn infer_dep_version(
        repo: &dyn RepoView,
        manifest: &Utf8Path,
        dep_path: &str,
    ) -> Option<String> {
        // 1) Target crate Cargo.toml
        let base = manifest.parent().unwrap_or_else(|| Utf8Path::new(""));
        let target_manifest: Utf8PathBuf = base.join(dep_path).join("Cargo.toml");

        if let Ok(contents) = repo.read_to_string(&target_manifest)
            && let Ok(doc) = contents.parse::<DocumentMut>()
            && let Some(pkg) = doc.get("package").and_then(|i| i.as_table())
            && let Some(v) = pkg
                .get("version")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
        {
            return Some(v.to_string());
        }

        // 2) Workspace package version, if present.
        if let Ok(contents) = repo.read_to_string(Utf8Path::new("Cargo.toml"))
            && let Ok(doc) = contents.parse::<DocumentMut>()
        {
            let ws = doc.get("workspace").and_then(|i| i.as_table());
            let ws_pkg = ws.and_then(|w| w.get("package")).and_then(|i| i.as_table());
            if let Some(v) = ws_pkg
                .and_then(|p| p.get("version"))
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
            {
                return Some(v.to_string());
            }
        }

        None
    }

    fn collect_path_deps(doc: &DocumentMut) -> Vec<PathDepCandidate> {
        let mut out = Vec::new();

        for (tbl_name, prefix) in [
            ("dependencies", vec!["dependencies".to_string()]),
            ("dev-dependencies", vec!["dev-dependencies".to_string()]),
            ("build-dependencies", vec!["build-dependencies".to_string()]),
        ] {
            if let Some(tbl) = doc.get(tbl_name).and_then(|i| i.as_table()) {
                out.extend(Self::collect_from_dep_table(tbl, prefix));
            }
        }

        // target.'cfg(...)'.dependencies
        if let Some(target) = doc.get("target").and_then(|i| i.as_table()) {
            for (target_key, target_item) in target.iter() {
                let Some(target_tbl) = target_item.as_table() else {
                    continue;
                };
                let target_name = target_key.to_string();

                for (tbl_name, prefix) in [
                    (
                        "dependencies",
                        vec![
                            "target".to_string(),
                            target_name.clone(),
                            "dependencies".to_string(),
                        ],
                    ),
                    (
                        "dev-dependencies",
                        vec![
                            "target".to_string(),
                            target_name.clone(),
                            "dev-dependencies".to_string(),
                        ],
                    ),
                    (
                        "build-dependencies",
                        vec![
                            "target".to_string(),
                            target_name.clone(),
                            "build-dependencies".to_string(),
                        ],
                    ),
                ] {
                    if let Some(dep_tbl) = target_tbl.get(tbl_name).and_then(|i| i.as_table()) {
                        out.extend(Self::collect_from_dep_table(dep_tbl, prefix));
                    }
                }
            }
        }

        out
    }

    fn collect_from_dep_table(tbl: &Table, prefix: Vec<String>) -> Vec<PathDepCandidate> {
        let mut out = Vec::new();
        for (dep_key, dep_item) in tbl.iter() {
            let dep_name = dep_key.to_string();

            // dep = { path = "../x" }
            if let Some(inline) = dep_item.as_inline_table() {
                let path = inline.get("path").and_then(|v| v.as_str());
                let version = inline.get("version").and_then(|v| v.as_str());
                let workspace_true = inline
                    .get("workspace")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if let Some(path) = path
                    && version.is_none()
                    && !workspace_true
                {
                    let mut toml_path = prefix.clone();
                    toml_path.push(dep_name.clone());
                    out.push(PathDepCandidate {
                        dep: dep_name,
                        dep_path: path.to_string(),
                        toml_path,
                    });
                }
                continue;
            }

            // [dependencies.dep] style
            if let Some(dep_tbl) = dep_item.as_table() {
                let path = dep_tbl
                    .get("path")
                    .and_then(|i| i.as_value())
                    .and_then(|v| v.as_str());
                let version = dep_tbl
                    .get("version")
                    .and_then(|i| i.as_value())
                    .and_then(|v| v.as_str());
                let workspace_true = dep_tbl
                    .get("workspace")
                    .and_then(|i| i.as_value())
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if let Some(path) = path
                    && version.is_none()
                    && !workspace_true
                {
                    let mut toml_path = prefix.clone();
                    toml_path.push(dep_name.clone());
                    out.push(PathDepCandidate {
                        dep: dep_name,
                        dep_path: path.to_string(),
                        toml_path,
                    });
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone)]
struct PathDepCandidate {
    dep: String,
    dep_path: String,
    toml_path: Vec<String>,
}

impl Fixer for PathDepVersionFixer {
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
        let triggers =
            receipts.matching_findings(Self::SENSORS, Self::CHECK_IDS, &["missing_version"]);
        if triggers.is_empty() {
            return Ok(vec![]);
        }

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

            let doc = match contents.parse::<DocumentMut>() {
                Ok(d) => d,
                Err(_) => continue,
            };

            let candidates = Self::collect_path_deps(&doc);
            for cand in candidates {
                let version = Self::infer_dep_version(repo, &manifest, &cand.dep_path);
                let safety = if version.is_some() {
                    SafetyClass::Safe
                } else {
                    SafetyClass::Unsafe
                };

                let mut args = serde_json::Map::new();
                args.insert(
                    "toml_path".to_string(),
                    serde_json::Value::Array(
                        cand.toml_path
                            .iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                );
                args.insert(
                    "dep".to_string(),
                    serde_json::Value::String(cand.dep.clone()),
                );
                args.insert(
                    "dep_path".to_string(),
                    serde_json::Value::String(cand.dep_path.clone()),
                );
                if let Some(v) = &version {
                    args.insert("version".to_string(), serde_json::Value::String(v.clone()));
                } else {
                    args.insert("version".to_string(), serde_json::Value::Null);
                }

                let manifest_path = manifest.to_string();
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
                        path: manifest_path,
                    },
                    kind: OpKind::TomlTransform {
                        rule_id: "ensure_path_dep_has_version".to_string(),
                        args: Some(serde_json::Value::Object(args)),
                    },
                    rationale: Rationale {
                        fix_key,
                        description: Some(Self::DESCRIPTION.to_string()),
                        findings,
                    },
                    params_required: if version.is_some() {
                        vec![]
                    } else {
                        vec!["version".to_string()]
                    },
                    preview: None,
                });
            }
        }

        Ok(fixes)
    }
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
            let raw = if rel.is_absolute() {
                rel.strip_prefix(&self.root)
                    .unwrap_or(rel)
                    .to_string()
            } else {
                rel.to_string()
            };
            raw.replace('\\', "/")
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
                name: "depguard".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("deps.path_requires_version".to_string()),
                code: Some("missing_version".to_string()),
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
            path: Utf8PathBuf::from("artifacts/depguard/report.json"),
            sensor_id: "depguard".to_string(),
            receipt: Ok(receipt),
        }];
        ReceiptSet::from_loaded(&loaded)
    }

    #[test]
    fn collect_path_deps_filters_expected_candidates() {
        let doc = r#"
            [dependencies]
            foo = { path = "../foo" }
            bar = { path = "../bar", version = "1.0" }
            baz = "1.0"
            qux = { path = "../qux", workspace = true }

            [dev-dependencies]
            devfoo = { path = "../devfoo" }

            [target.'cfg(windows)'.dependencies]
            winfoo = { path = "../winfoo" }
        "#
        .parse::<DocumentMut>()
        .expect("parse");

        let candidates = PathDepVersionFixer::collect_path_deps(&doc);
        let paths: Vec<Vec<String>> =
            candidates.iter().map(|c| c.toml_path.clone()).collect();

        assert!(paths.contains(&vec!["dependencies".to_string(), "foo".to_string()]));
        assert!(paths.contains(&vec!["dev-dependencies".to_string(), "devfoo".to_string()]));
        assert!(paths.contains(&vec![
            "target".to_string(),
            "cfg(windows)".to_string(),
            "dependencies".to_string(),
            "winfoo".to_string()
        ]));
        assert_eq!(candidates.len(), 3);
    }

    #[test]
    fn infer_dep_version_prefers_target_manifest() {
        let repo = TestRepo::new(&[
            ("crates/app/Cargo.toml", "[package]\nname = \"app\"\n"),
            (
                "crates/app/dep/Cargo.toml",
                "[package]\nname = \"dep\"\nversion = \"0.2.0\"\n",
            ),
        ]);

        let manifest = Utf8Path::new("crates/app/Cargo.toml");
        let version = PathDepVersionFixer::infer_dep_version(&repo, manifest, "dep");
        assert_eq!(version.as_deref(), Some("0.2.0"));
    }

    #[test]
    fn infer_dep_version_falls_back_to_workspace_package() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [workspace.package]
                version = "1.5.0"
            "#,
        )]);

        let manifest = Utf8Path::new("crates/app/Cargo.toml");
        let version = PathDepVersionFixer::infer_dep_version(&repo, manifest, "../dep");
        assert_eq!(version.as_deref(), Some("1.5.0"));
    }

    #[test]
    fn plan_emits_guarded_fix_when_version_known() {
        let repo = TestRepo::new(&[
            ("Cargo.toml", "[workspace.package]\nversion = \"1.2.3\"\n"),
            (
                "crates/app/Cargo.toml",
                "[package]\nname = \"app\"\n\n[dependencies]\ndep = { path = \"../dep\" }\n",
            ),
        ]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let receipt_set = receipt_set_for("crates/app/Cargo.toml");
        let fixes = PathDepVersionFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        let op = &fixes[0];
        assert_eq!(op.safety, SafetyClass::Safe);
        assert!(matches!(op.kind, OpKind::TomlTransform { .. }));
        if let OpKind::TomlTransform { rule_id, args } = &op.kind {
            assert_eq!(rule_id, "ensure_path_dep_has_version");
            assert_eq!(args.as_ref().unwrap()["version"], "1.2.3");
        }
    }

    #[test]
    fn plan_emits_unsafe_fix_when_version_unknown() {
        let repo = TestRepo::new(&[(
            "crates/app/Cargo.toml",
            "[package]\nname = \"app\"\n\n[dependencies]\ndep = { path = \"../dep\" }\n",
        )]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let receipt_set = receipt_set_for("crates/app/Cargo.toml");
        let fixes = PathDepVersionFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        let op = &fixes[0];
        assert_eq!(op.safety, SafetyClass::Unsafe);
        assert_eq!(op.params_required, vec!["version".to_string()]);
    }

    #[test]
    fn collect_path_deps_includes_table_style_and_skips_workspace() {
        let doc = r#"
            [dependencies.dep]
            path = "../dep"

            [dependencies.ws]
            path = "../ws"
            workspace = true
        "#
        .parse::<DocumentMut>()
        .expect("parse");

        let candidates = PathDepVersionFixer::collect_path_deps(&doc);
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].toml_path,
            vec!["dependencies".to_string(), "dep".to_string()]
        );
    }

    #[test]
    fn collect_path_deps_skips_non_table_target_entries() {
        let doc = r#"
            [target]
            "cfg(windows)" = "noop"
        "#
        .parse::<DocumentMut>()
        .expect("parse");

        let candidates = PathDepVersionFixer::collect_path_deps(&doc);
        assert!(candidates.is_empty());
    }

    #[test]
    fn plan_skips_missing_or_invalid_manifest() {
        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let repo_missing = TestRepo::new(&[]);
        let receipt_set = receipt_set_for("crates/app/Cargo.toml");
        let fixes = PathDepVersionFixer
            .plan(&ctx, &repo_missing, &receipt_set)
            .expect("plan");
        assert!(fixes.is_empty());

        let repo_invalid = TestRepo::new(&[("crates/app/Cargo.toml", "not toml = [")]);
        let fixes = PathDepVersionFixer
            .plan(&ctx, &repo_invalid, &receipt_set)
            .expect("plan");
        assert!(fixes.is_empty());
    }

    #[test]
    fn test_repo_helpers_handle_absolute_paths() {
        let root = Utf8PathBuf::from_path_buf(std::env::current_dir().expect("cwd"))
            .expect("utf8");
        let mut files = HashMap::new();
        files.insert("crates/app/Cargo.toml".to_string(), "name = \"app\"".to_string());
        let repo = TestRepo { root: root.clone(), files };
        let abs = root.join("crates/app/Cargo.toml");
        assert!(repo.exists(&abs));
        assert_eq!(repo.read_to_string(&abs).unwrap(), "name = \"app\"");
    }

    #[test]
    fn fix_key_for_handles_missing_check_id() {
        let f = buildfix_types::plan::FindingRef {
            source: "depguard".to_string(),
            check_id: None,
            code: "X".to_string(),
            path: None,
            line: None,
            fingerprint: None,
        };
        assert_eq!(super::fix_key_for(&f), "depguard/-/X");
    }
}

fn fix_key_for(f: &buildfix_types::plan::FindingRef) -> String {
    let check = f.check_id.clone().unwrap_or_else(|| "-".to_string());
    format!("{}/{}/{}", f.source, check, f.code)
}
