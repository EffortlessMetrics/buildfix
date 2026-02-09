use crate::fixers::{Fixer, FixerMeta};
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{PlanOp, Rationale};
use camino::Utf8PathBuf;
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::{DocumentMut, Table};

pub struct WorkspaceInheritanceFixer;

impl WorkspaceInheritanceFixer {
    const FIX_ID: &'static str = "cargo.use_workspace_dependency";
    const DESCRIPTION: &'static str = "Converts dependency specs to workspace = true inheritance";
    const SENSORS: &'static [&'static str] = &["depguard"];
    const CHECK_IDS: &'static [&'static str] =
        &["deps.workspace_inheritance", "cargo.workspace_inheritance"];

    fn workspace_deps(repo: &dyn RepoView) -> BTreeMap<String, WorkspaceDepSpec> {
        let Ok(contents) = repo.read_to_string("Cargo.toml".as_ref()) else {
            return BTreeMap::new();
        };
        let Ok(doc) = contents.parse::<DocumentMut>() else {
            return BTreeMap::new();
        };

        let Some(ws) = doc.get("workspace").and_then(|i| i.as_table()) else {
            return BTreeMap::new();
        };
        let Some(deps) = ws.get("dependencies").and_then(|i| i.as_table()) else {
            return BTreeMap::new();
        };

        let mut out = BTreeMap::new();
        for (k, item) in deps.iter() {
            let name = k.to_string();
            let mut spec = WorkspaceDepSpec::default();

            // Inline tables are represented as values, so check them first.
            if let Some(inline) = item.as_inline_table() {
                if inline.get("path").is_some() || inline.get("git").is_some() {
                    spec.is_path_or_git = true;
                }
                if let Some(v) = inline.get("version").and_then(|v| v.as_str()) {
                    spec.version = Some(v.to_string());
                }
            } else if let Some(tbl) = item.as_table() {
                if tbl.get("path").is_some() || tbl.get("git").is_some() {
                    spec.is_path_or_git = true;
                }
                if let Some(v) = tbl
                    .get("version")
                    .and_then(|i| i.as_value())
                    .and_then(|v| v.as_str())
                {
                    spec.version = Some(v.to_string());
                }
            } else if let Some(v) = item.as_value().and_then(|v| v.as_str()) {
                spec.version = Some(v.to_string());
            }

            out.insert(name, spec);
        }

        out
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

    fn collect_candidates(
        doc: &DocumentMut,
        workspace_deps: &BTreeMap<String, WorkspaceDepSpec>,
    ) -> Vec<WorkspaceDepCandidate> {
        let mut out = Vec::new();

        for (tbl_name, prefix) in [
            ("dependencies", vec!["dependencies".to_string()]),
            ("dev-dependencies", vec!["dev-dependencies".to_string()]),
            ("build-dependencies", vec!["build-dependencies".to_string()]),
        ] {
            if let Some(tbl) = doc.get(tbl_name).and_then(|i| i.as_table()) {
                out.extend(Self::collect_from_dep_table(tbl, &prefix, workspace_deps));
            }
        }

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
                        out.extend(Self::collect_from_dep_table(
                            dep_tbl,
                            &prefix,
                            workspace_deps,
                        ));
                    }
                }
            }
        }

        out
    }

    fn collect_from_dep_table(
        tbl: &Table,
        prefix: &[String],
        workspace_deps: &BTreeMap<String, WorkspaceDepSpec>,
    ) -> Vec<WorkspaceDepCandidate> {
        let mut out = Vec::new();

        for (dep_key, dep_item) in tbl.iter() {
            let dep = dep_key.to_string();
            let Some(ws_spec) = workspace_deps.get(&dep) else {
                continue;
            };

            // dep = "1.2.3"
            if dep_item.is_value() && dep_item.as_value().and_then(|v| v.as_str()).is_some() {
                let mut toml_path = prefix.to_vec();
                toml_path.push(dep.clone());
                let member_version = dep_item
                    .as_value()
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                out.push(WorkspaceDepCandidate {
                    dep,
                    toml_path,
                    preserved: DepPreserve::default(),
                    member_version,
                    workspace_spec: ws_spec.clone(),
                });
                continue;
            }

            // dep = { version = "...", features = [...] }
            if let Some(inline) = dep_item.as_inline_table() {
                let workspace_true = inline
                    .get("workspace")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if workspace_true {
                    continue;
                }

                // Skip if this isn't a plain registry dep.
                if inline.get("path").is_some() || inline.get("git").is_some() {
                    continue;
                }

                let preserved = DepPreserve {
                    package: inline
                        .get("package")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    optional: inline.get("optional").and_then(|v| v.as_bool()),
                    default_features: inline.get("default-features").and_then(|v| v.as_bool()),
                    features: inline
                        .get("features")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                };

                let mut toml_path = prefix.to_vec();
                toml_path.push(dep.clone());
                let member_version = inline
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                out.push(WorkspaceDepCandidate {
                    dep,
                    toml_path,
                    preserved,
                    member_version,
                    workspace_spec: ws_spec.clone(),
                });
                continue;
            }

            // [dependencies.dep] style
            if let Some(dep_tbl) = dep_item.as_table() {
                let workspace_true = dep_tbl
                    .get("workspace")
                    .and_then(|i| i.as_value())
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if workspace_true {
                    continue;
                }

                if dep_tbl.get("path").is_some() || dep_tbl.get("git").is_some() {
                    continue;
                }

                let preserved = DepPreserve {
                    package: dep_tbl
                        .get("package")
                        .and_then(|i| i.as_value())
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    optional: dep_tbl
                        .get("optional")
                        .and_then(|i| i.as_value())
                        .and_then(|v| v.as_bool()),
                    default_features: dep_tbl
                        .get("default-features")
                        .and_then(|i| i.as_value())
                        .and_then(|v| v.as_bool()),
                    features: dep_tbl
                        .get("features")
                        .and_then(|i| i.as_value())
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                };

                let mut toml_path = prefix.to_vec();
                toml_path.push(dep.clone());
                let member_version = dep_tbl
                    .get("version")
                    .and_then(|i| i.as_value())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                out.push(WorkspaceDepCandidate {
                    dep,
                    toml_path,
                    preserved,
                    member_version,
                    workspace_spec: ws_spec.clone(),
                });
            }
        }

        out
    }
}

#[derive(Debug, Clone)]
struct WorkspaceDepCandidate {
    dep: String,
    toml_path: Vec<String>,
    preserved: DepPreserve,
    member_version: Option<String>,
    workspace_spec: WorkspaceDepSpec,
}

#[derive(Debug, Clone, Default)]
struct WorkspaceDepSpec {
    version: Option<String>,
    is_path_or_git: bool,
}

#[derive(Debug, Clone, Default)]
struct DepPreserve {
    package: Option<String>,
    optional: Option<bool>,
    default_features: Option<bool>,
    features: Vec<String>,
}

impl Fixer for WorkspaceInheritanceFixer {
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

        let workspace_deps = Self::workspace_deps(repo);
        if workspace_deps.is_empty() {
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

            for cand in Self::collect_candidates(&doc, &workspace_deps) {
                let mut safety = SafetyClass::Safe;

                if cand.workspace_spec.is_path_or_git {
                    safety = SafetyClass::Guarded;
                } else if let Some(member_version) = &cand.member_version {
                    match &cand.workspace_spec.version {
                        Some(ws_version) if ws_version != member_version => {
                            safety = SafetyClass::Guarded;
                        }
                        None => {
                            safety = SafetyClass::Guarded;
                        }
                        _ => {}
                    }
                }

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

                let mut preserved = serde_json::Map::new();
                if let Some(pkg) = &cand.preserved.package {
                    preserved.insert(
                        "package".to_string(),
                        serde_json::Value::String(pkg.clone()),
                    );
                }
                if let Some(opt) = cand.preserved.optional {
                    preserved.insert("optional".to_string(), serde_json::Value::Bool(opt));
                }
                if let Some(df) = cand.preserved.default_features {
                    preserved.insert("default_features".to_string(), serde_json::Value::Bool(df));
                }
                if !cand.preserved.features.is_empty() {
                    preserved.insert(
                        "features".to_string(),
                        serde_json::Value::Array(
                            cand.preserved
                                .features
                                .iter()
                                .map(|s| serde_json::Value::String(s.clone()))
                                .collect(),
                        ),
                    );
                }
                args.insert(
                    "preserved".to_string(),
                    serde_json::Value::Object(preserved),
                );

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
                        rule_id: "use_workspace_dependency".to_string(),
                        args: Some(serde_json::Value::Object(args)),
                    },
                    rationale: Rationale {
                        fix_key,
                        description: Some(Self::DESCRIPTION.to_string()),
                        findings,
                    },
                    params_required: vec![],
                    preview: None,
                });
            }
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
    use std::collections::{BTreeMap, HashMap};

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
                name: "depguard".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("deps.workspace_inheritance".to_string()),
                code: Some("WS_INHERIT".to_string()),
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
    fn workspace_deps_parses_versions_and_paths() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [workspace.dependencies]
                serde = "1.0"
                local = { path = "../local" }
                gitdep = { git = "https://example.com/repo.git" }
            "#,
        )]);

        let deps = WorkspaceInheritanceFixer::workspace_deps(&repo);
        assert_eq!(deps.get("serde").unwrap().version.as_deref(), Some("1.0"));
        assert!(!deps.get("serde").unwrap().is_path_or_git);
        assert!(deps.get("local").unwrap().is_path_or_git);
        assert!(deps.get("gitdep").unwrap().is_path_or_git);
    }

    #[test]
    fn plan_emits_ops_with_expected_safety_and_preserved_fields() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.dependencies]
                    serde = "1.0"
                    local = { path = "../local" }
                "#,
            ),
            (
                "crates/member/Cargo.toml",
                r#"
                    [package]
                    name = "member"

                    [dependencies]
                    serde = "1.0"
                    local = { version = "0.1", optional = true, features = ["std"] }
                "#,
            ),
        ]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let receipt_set = receipt_set_for("crates/member/Cargo.toml");
        let fixes = WorkspaceInheritanceFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 2);

        let mut by_dep: HashMap<String, &PlanOp> = HashMap::new();
        for op in &fixes {
            if let OpKind::TomlTransform {
                args: Some(args), ..
            } = &op.kind
            {
                if let Some(dep) = args.get("dep").and_then(|d| d.as_str()) {
                    by_dep.insert(dep.to_string(), op);
                }
            }
        }

        let serde_op = by_dep.get("serde").expect("serde op");
        assert_eq!(serde_op.safety, SafetyClass::Safe);

        let local_op = by_dep.get("local").expect("local op");
        assert_eq!(local_op.safety, SafetyClass::Guarded);
        if let OpKind::TomlTransform {
            args: Some(args), ..
        } = &local_op.kind
        {
            let preserved = &args["preserved"];
            assert_eq!(preserved["optional"], serde_json::json!(true));
            assert_eq!(preserved["features"], serde_json::json!(["std"]));
        } else {
            panic!("expected transform");
        }
    }

    #[test]
    fn workspace_deps_returns_empty_for_missing_or_invalid() {
        let repo_missing = TestRepo::new(&[]);
        assert!(WorkspaceInheritanceFixer::workspace_deps(&repo_missing).is_empty());

        let repo_invalid = TestRepo::new(&[("Cargo.toml", "not toml = [")]);
        assert!(WorkspaceInheritanceFixer::workspace_deps(&repo_invalid).is_empty());

        let repo_no_ws = TestRepo::new(&[("Cargo.toml", "[package]\nname = \"demo\"")]);
        assert!(WorkspaceInheritanceFixer::workspace_deps(&repo_no_ws).is_empty());

        let repo_no_deps = TestRepo::new(&[("Cargo.toml", "[workspace]\n")]);
        assert!(WorkspaceInheritanceFixer::workspace_deps(&repo_no_deps).is_empty());
    }

    #[test]
    fn collect_candidates_includes_target_dependencies() {
        let doc = r#"
            [target."cfg(windows)".dependencies]
            serde = "1.0"
        "#
        .parse::<DocumentMut>()
        .expect("parse");

        let mut ws = BTreeMap::new();
        ws.insert(
            "serde".to_string(),
            WorkspaceDepSpec {
                version: Some("1.0".to_string()),
                is_path_or_git: false,
            },
        );

        let cands = WorkspaceInheritanceFixer::collect_candidates(&doc, &ws);
        assert!(cands.iter().any(|c| {
            c.toml_path
                == vec![
                    "target".to_string(),
                    "cfg(windows)".to_string(),
                    "dependencies".to_string(),
                    "serde".to_string(),
                ]
        }));
    }

    #[test]
    fn collect_candidates_skips_workspace_and_path_deps() {
        let doc = r#"
            [dependencies]
            workspace_dep = { workspace = true }
            local = { path = "../local" }
        "#
        .parse::<DocumentMut>()
        .expect("parse");

        let mut ws = BTreeMap::new();
        ws.insert(
            "workspace_dep".to_string(),
            WorkspaceDepSpec {
                version: Some("1.0".to_string()),
                is_path_or_git: false,
            },
        );
        ws.insert(
            "local".to_string(),
            WorkspaceDepSpec {
                version: None,
                is_path_or_git: true,
            },
        );

        let cands = WorkspaceInheritanceFixer::collect_candidates(&doc, &ws);
        assert!(cands.is_empty());
    }

    #[test]
    fn collect_candidates_preserves_table_style_fields() {
        let doc = r#"
            [dependencies.dep]
            version = "1.0"
            package = "dep-pkg"
            optional = true
            default-features = false
            features = ["std", "serde"]
        "#
        .parse::<DocumentMut>()
        .expect("parse");

        let mut ws = BTreeMap::new();
        ws.insert(
            "dep".to_string(),
            WorkspaceDepSpec {
                version: Some("1.0".to_string()),
                is_path_or_git: false,
            },
        );

        let cands = WorkspaceInheritanceFixer::collect_candidates(&doc, &ws);
        assert_eq!(cands.len(), 1);
        let cand = &cands[0];
        assert_eq!(cand.preserved.package.as_deref(), Some("dep-pkg"));
        assert_eq!(cand.preserved.optional, Some(true));
        assert_eq!(cand.preserved.default_features, Some(false));
        assert_eq!(
            cand.preserved.features,
            vec!["std".to_string(), "serde".to_string()]
        );
    }

    #[test]
    fn plan_marks_guarded_for_version_mismatch() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.dependencies]
                    serde = "1.0"
                "#,
            ),
            (
                "crates/member/Cargo.toml",
                r#"
                    [package]
                    name = "member"

                    [dependencies]
                    serde = "2.0"
                "#,
            ),
        ]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let receipt_set = receipt_set_for("crates/member/Cargo.toml");
        let fixes = WorkspaceInheritanceFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].safety, SafetyClass::Guarded);
    }

    #[test]
    fn plan_skips_missing_or_invalid_manifest() {
        let repo_missing =
            TestRepo::new(&[("Cargo.toml", "[workspace]\nmembers = [\"crates/member\"]\n")]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let receipt_set = receipt_set_for("crates/member/Cargo.toml");
        let fixes = WorkspaceInheritanceFixer
            .plan(&ctx, &repo_missing, &receipt_set)
            .expect("plan");
        assert!(fixes.is_empty());

        let repo_invalid = TestRepo::new(&[
            ("Cargo.toml", "[workspace]\n"),
            ("crates/member/Cargo.toml", "not toml = ["),
        ]);
        let fixes = WorkspaceInheritanceFixer
            .plan(&ctx, &repo_invalid, &receipt_set)
            .expect("plan");
        assert!(fixes.is_empty());
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
        assert_eq!(fix_key_for(&f), "depguard/-/X");
    }
}
