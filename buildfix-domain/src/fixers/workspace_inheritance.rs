use crate::fixers::{Fixer, FixerMeta};
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{DepPreserve, FixId, Operation, SafetyClass};
use buildfix_types::plan::PlannedFix;
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

    fn workspace_deps(repo: &dyn RepoView) -> BTreeSet<String> {
        let Ok(contents) = repo.read_to_string("Cargo.toml".as_ref()) else {
            return BTreeSet::new();
        };
        let Ok(doc) = contents.parse::<DocumentMut>() else {
            return BTreeSet::new();
        };

        let Some(ws) = doc.get("workspace").and_then(|i| i.as_table()) else {
            return BTreeSet::new();
        };
        let Some(deps) = ws.get("dependencies").and_then(|i| i.as_table()) else {
            return BTreeSet::new();
        };

        deps.iter().map(|(k, _)| k.to_string()).collect()
    }

    fn manifest_paths_from_triggers(
        triggers: &[buildfix_types::plan::FindingRef],
    ) -> BTreeSet<Utf8PathBuf> {
        let mut out = BTreeSet::new();
        for t in triggers {
            let Some(loc) = &t.location else { continue };
            if loc.path.as_str().ends_with("Cargo.toml") {
                out.insert(loc.path.clone());
            }
        }
        out
    }

    fn collect_candidates(
        doc: &DocumentMut,
        workspace_deps: &BTreeSet<String>,
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
        workspace_deps: &BTreeSet<String>,
    ) -> Vec<WorkspaceDepCandidate> {
        let mut out = Vec::new();

        for (dep_key, dep_item) in tbl.iter() {
            let dep = dep_key.to_string();
            if !workspace_deps.contains(&dep) {
                continue;
            }

            // dep = "1.2.3"
            if dep_item.is_value() && dep_item.as_value().and_then(|v| v.as_str()).is_some() {
                let mut toml_path = prefix.to_vec();
                toml_path.push(dep.clone());
                out.push(WorkspaceDepCandidate {
                    dep,
                    toml_path,
                    preserved: DepPreserve::default(),
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
                out.push(WorkspaceDepCandidate {
                    dep,
                    toml_path,
                    preserved,
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
                out.push(WorkspaceDepCandidate {
                    dep,
                    toml_path,
                    preserved,
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
    ) -> anyhow::Result<Vec<PlannedFix>> {
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
            if let Some(loc) = &t.location {
                triggers_by_manifest
                    .entry(loc.path.clone())
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
                let title = format!("Use workspace = true for dependency {}", cand.dep);
                fixes.push(PlannedFix {
                    id: String::new(),
                    fix_id: FixId::new(Self::FIX_ID),
                    safety: SafetyClass::Safe,
                    title,
                    description: Some(
                        "Converts member dependency specs to inherit from [workspace.dependencies]."
                            .to_string(),
                    ),
                    triggers: triggers_by_manifest
                        .get(&manifest)
                        .cloned()
                        .unwrap_or_else(Vec::new),
                    operations: vec![Operation::UseWorkspaceDependency {
                        manifest: manifest.clone(),
                        toml_path: cand.toml_path.clone(),
                        dep: cand.dep.clone(),
                        preserved: cand.preserved.clone(),
                    }],
                    preconditions: vec![],
                });
            }
        }

        Ok(fixes)
    }
}
