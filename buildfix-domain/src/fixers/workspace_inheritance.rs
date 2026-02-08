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
