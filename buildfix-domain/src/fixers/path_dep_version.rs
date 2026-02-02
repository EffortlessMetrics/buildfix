use crate::fixers::Fixer;
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{FixId, Operation, SafetyClass};
use buildfix_types::plan::PlannedFix;
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::{DocumentMut, Table};

pub struct PathDepVersionFixer;

impl PathDepVersionFixer {
    const FIX_ID: &'static str = "cargo.path_dep_add_version";

    fn manifest_paths_from_triggers(triggers: &[buildfix_types::plan::FindingRef]) -> BTreeSet<Utf8PathBuf> {
        let mut out = BTreeSet::new();
        for t in triggers {
            let Some(loc) = &t.location else { continue };
            if loc.path.as_str().ends_with("Cargo.toml") {
                out.insert(loc.path.clone());
            }
        }
        out
    }

    fn infer_dep_version(repo: &dyn RepoView, manifest: &Utf8Path, dep_path: &str) -> Option<String> {
        // 1) Target crate Cargo.toml
        let base = manifest.parent().unwrap_or_else(|| Utf8Path::new(""));
        let target_manifest: Utf8PathBuf = base.join(dep_path).join("Cargo.toml");

        if let Ok(contents) = repo.read_to_string(&target_manifest) {
            if let Ok(doc) = contents.parse::<DocumentMut>() {
                if let Some(pkg) = doc.get("package").and_then(|i| i.as_table()) {
                    if let Some(v) = pkg
                        .get("version")
                        .and_then(|i| i.as_value())
                        .and_then(|v| v.as_str())
                    {
                        return Some(v.to_string());
                    }
                }
            }
        }

        // 2) Workspace package version, if present.
        if let Ok(contents) = repo.read_to_string(Utf8Path::new("Cargo.toml")) {
            if let Ok(doc) = contents.parse::<DocumentMut>() {
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
                let Some(target_tbl) = target_item.as_table() else { continue };
                let target_name = target_key.to_string();

                for (tbl_name, prefix) in [
                    ("dependencies", vec!["target".to_string(), target_name.clone(), "dependencies".to_string()]),
                    ("dev-dependencies", vec!["target".to_string(), target_name.clone(), "dev-dependencies".to_string()]),
                    ("build-dependencies", vec!["target".to_string(), target_name.clone(), "build-dependencies".to_string()]),
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
                let workspace_true = inline.get("workspace").and_then(|v| v.as_bool()).unwrap_or(false);

                if let Some(path) = path {
                    if version.is_none() && !workspace_true {
                        let mut toml_path = prefix.clone();
                        toml_path.push(dep_name.clone());
                        out.push(PathDepCandidate {
                            dep: dep_name,
                            dep_path: path.to_string(),
                            toml_path,
                        });
                    }
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

                if let Some(path) = path {
                    if version.is_none() && !workspace_true {
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
    fn plan(
        &self,
        _ctx: &crate::planner::PlanContext,
        repo: &dyn RepoView,
        receipts: &ReceiptSet,
    ) -> anyhow::Result<Vec<PlannedFix>> {
        let triggers = receipts.matching_findings(
            &["depguard"],
            &["deps.path_requires_version", "cargo.path_requires_version"],
            &["missing_version"],
        );
        if triggers.is_empty() {
            return Ok(vec![]);
        }

        let mut triggers_by_manifest: BTreeMap<Utf8PathBuf, Vec<buildfix_types::plan::FindingRef>> = BTreeMap::new();
        for t in &triggers {
            if let Some(loc) = &t.location {
                triggers_by_manifest.entry(loc.path.clone()).or_default().push(t.clone());
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
                let Some(version) = Self::infer_dep_version(repo, &manifest, &cand.dep_path) else {
                    // If we can't infer a version deterministically, skip. This keeps the fix set
                    // "safe by default".
                    continue;
                };

                let title = format!(
                    "Add version = "{}" for path dependency {}",
                    version, cand.dep
                );

                fixes.push(PlannedFix {
                    id: String::new(),
                    fix_id: FixId::new(Self::FIX_ID),
                    safety: SafetyClass::Safe,
                    title,
                    description: Some(format!(
                        "Adds a version field so the path dependency is publishable / policy-compliant."
                    )),
                    triggers: triggers_by_manifest
                        .get(&manifest)
                        .cloned()
                        .unwrap_or_else(Vec::new),
                    operations: vec![Operation::EnsurePathDepHasVersion {
                        manifest: manifest.clone(),
                        toml_path: cand.toml_path.clone(),
                        dep: cand.dep.clone(),
                        dep_path: cand.dep_path.clone(),
                        version,
                    }],
                    preconditions: vec![],
                });
            }
        }

        Ok(fixes)
    }
}
