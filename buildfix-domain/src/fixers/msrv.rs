use crate::fixers::Fixer;
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{FixId, Operation, SafetyClass};
use buildfix_types::plan::PlannedFix;
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::DocumentMut;

pub struct MsrvNormalizeFixer;

impl MsrvNormalizeFixer {
    const FIX_ID: &'static str = "cargo.normalize_rust_version";

    fn canonical_rust_version(repo: &dyn RepoView) -> Option<String> {
        let contents = repo.read_to_string(Utf8Path::new("Cargo.toml")).ok()?;
        let doc = contents.parse::<DocumentMut>().ok()?;

        // Preferred: [workspace.package].rust-version
        if let Some(ws) = doc.get("workspace").and_then(|i| i.as_table()) {
            if let Some(pkg) = ws.get("package").and_then(|i| i.as_table()) {
                if let Some(v) = pkg
                    .get("rust-version")
                    .and_then(|i| i.as_value())
                    .and_then(|v| v.as_str())
                {
                    return Some(v.to_string());
                }
            }
        }

        // Fallback: [package].rust-version
        if let Some(pkg) = doc.get("package").and_then(|i| i.as_table()) {
            if let Some(v) = pkg
                .get("rust-version")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
            {
                return Some(v.to_string());
            }
        }

        None
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

    fn needs_change(contents: &str, rust_version: &str) -> bool {
        let Ok(doc) = contents.parse::<DocumentMut>() else { return true };
        let Some(pkg) = doc.get("package").and_then(|i| i.as_table()) else { return true };

        let current = pkg
            .get("rust-version")
            .and_then(|i| i.as_value())
            .and_then(|v| v.as_str());

        current != Some(rust_version)
    }
}

impl Fixer for MsrvNormalizeFixer {
    fn plan(
        &self,
        _ctx: &crate::planner::PlanContext,
        repo: &dyn RepoView,
        receipts: &ReceiptSet,
    ) -> anyhow::Result<Vec<PlannedFix>> {
        let triggers = receipts.matching_findings(
            &["builddiag", "cargo"],
            &["rust.msrv_consistent", "cargo.msrv_consistent", "msrv.consistent"],
            &[],
        );
        if triggers.is_empty() {
            return Ok(vec![]);
        }

        let Some(rust_version) = Self::canonical_rust_version(repo) else {
            return Ok(vec![]);
        };

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
            if !Self::needs_change(&contents, &rust_version) {
                continue;
            }

            fixes.push(PlannedFix {
                id: String::new(),
                fix_id: FixId::new(Self::FIX_ID),
                safety: SafetyClass::Guarded,
                title: format!("Set rust-version = "{}" in {}", rust_version, manifest),
                description: Some(
                    "Normalizes per-crate MSRV declarations to the workspace canonical value."
                        .to_string(),
                ),
                triggers: triggers_by_manifest
                    .get(&manifest)
                    .cloned()
                    .unwrap_or_else(Vec::new),
                operations: vec![Operation::SetPackageRustVersion {
                    manifest: manifest.clone(),
                    rust_version: rust_version.clone(),
                }],
                preconditions: vec![],
            });
        }

        Ok(fixes)
    }
}
