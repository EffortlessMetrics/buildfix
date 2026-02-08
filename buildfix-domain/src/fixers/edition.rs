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
