use crate::fixers::{Fixer, FixerMeta};
use crate::planner::{MatchedFinding, ReceiptSet};
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{FindingRef, PlanOp, Rationale};
use camino::Utf8PathBuf;
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::{DocumentMut, Item};

pub struct DuplicateDepsConsolidationFixer;

impl DuplicateDepsConsolidationFixer {
    const FIX_ID: &'static str = "cargo.consolidate_duplicate_deps";
    const DESCRIPTION: &'static str =
        "Consolidates duplicate dependency versions into workspace.dependencies";
    const SENSORS: &'static [&'static str] = &["depguard"];
    const CHECK_IDS: &'static [&'static str] = &[
        "deps.duplicate_dependency_versions",
        "cargo.duplicate_dependency_versions",
        "deps.duplicate_versions",
        "cargo.duplicate_versions",
    ];

    fn parse_receipt_candidate(matched: &MatchedFinding) -> Option<RawCandidate> {
        let path = matched.finding.path.as_ref()?;
        if !path.ends_with("Cargo.toml") {
            return None;
        }

        let data = matched.data.as_ref()?.as_object()?;
        let dep = data
            .get("dep")
            .or_else(|| data.get("dependency"))
            .and_then(|v| v.as_str())?
            .trim();
        if dep.is_empty() {
            return None;
        }

        let selected_version = data
            .get("selected_version")
            .or_else(|| data.get("workspace_version"))
            .or_else(|| data.get("version"))
            .and_then(|v| v.as_str())?
            .trim();
        if selected_version.is_empty() {
            return None;
        }

        let toml_path = data.get("toml_path").and_then(parse_toml_path)?;

        Some(RawCandidate {
            manifest: Utf8PathBuf::from(path.clone()),
            dep: dep.to_string(),
            selected_version: selected_version.to_string(),
            toml_path,
            finding: matched.finding.clone(),
        })
    }

    fn workspace_dep_exists(repo: &dyn RepoView, dep_name: &str) -> bool {
        let Ok(contents) = repo.read_to_string("Cargo.toml".as_ref()) else {
            return false;
        };
        let Ok(doc) = contents.parse::<DocumentMut>() else {
            return false;
        };
        doc.get("workspace")
            .and_then(|w| w.get("dependencies"))
            .and_then(|d| d.get(dep_name))
            .is_some()
    }

    fn enrich_candidate(repo: &dyn RepoView, raw: RawCandidate) -> Option<ConsolidationCandidate> {
        let contents = repo.read_to_string(&raw.manifest).ok()?;
        let doc = contents.parse::<DocumentMut>().ok()?;
        let dep_item = get_dep_item(&doc, &raw.toml_path)?;
        let preserved = dep_preserve_from_item(dep_item)?;

        Some(ConsolidationCandidate {
            manifest: raw.manifest,
            dep: raw.dep,
            selected_version: raw.selected_version,
            toml_path: raw.toml_path,
            preserved,
            finding: raw.finding,
        })
    }

    fn root_op(dep: &str, selected_version: &str, findings: Vec<FindingRef>) -> PlanOp {
        let mut args = serde_json::Map::new();
        args.insert(
            "dep".to_string(),
            serde_json::Value::String(dep.to_string()),
        );
        args.insert(
            "version".to_string(),
            serde_json::Value::String(selected_version.to_string()),
        );

        let fix_key = findings
            .first()
            .map(fix_key_for)
            .unwrap_or_else(|| "unknown/-/-".to_string());

        PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: "Cargo.toml".to_string(),
            },
            kind: OpKind::TomlTransform {
                rule_id: "ensure_workspace_dependency_version".to_string(),
                args: Some(serde_json::Value::Object(args)),
            },
            rationale: Rationale {
                fix_key,
                description: Some(Self::DESCRIPTION.to_string()),
                findings,
            },
            params_required: vec![],
            preview: None,
        }
    }

    fn member_op(cand: &ConsolidationCandidate) -> PlanOp {
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
            "preserved".to_string(),
            serde_json::Value::Object(cand.preserved.to_json()),
        );

        PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: cand.manifest.to_string(),
            },
            kind: OpKind::TomlTransform {
                rule_id: "use_workspace_dependency".to_string(),
                args: Some(serde_json::Value::Object(args)),
            },
            rationale: Rationale {
                fix_key: fix_key_for(&cand.finding),
                description: Some(Self::DESCRIPTION.to_string()),
                findings: vec![cand.finding.clone()],
            },
            params_required: vec![],
            preview: None,
        }
    }
}

impl Fixer for DuplicateDepsConsolidationFixer {
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
        let matched = receipts.matching_findings_with_data(Self::SENSORS, Self::CHECK_IDS, &[]);
        if matched.is_empty() {
            return Ok(vec![]);
        }

        let mut candidates = Vec::new();
        for m in &matched {
            let Some(raw) = Self::parse_receipt_candidate(m) else {
                continue;
            };
            if let Some(cand) = Self::enrich_candidate(repo, raw) {
                candidates.push(cand);
            }
        }

        if candidates.is_empty() {
            return Ok(vec![]);
        }

        let mut by_dep: BTreeMap<String, Vec<ConsolidationCandidate>> = BTreeMap::new();
        let mut selected_versions_by_dep: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for cand in candidates {
            selected_versions_by_dep
                .entry(cand.dep.clone())
                .or_default()
                .insert(cand.selected_version.clone());
            by_dep.entry(cand.dep.clone()).or_default().push(cand);
        }

        let mut ops = Vec::new();

        for (dep, mut dep_candidates) in by_dep {
            // Conflicting selected versions are ambiguous; skip safely.
            let Some(versions) = selected_versions_by_dep.get(&dep) else {
                continue;
            };
            if versions.len() != 1 {
                continue;
            }
            let selected_version = versions.iter().next().cloned().unwrap_or_default();
            if selected_version.is_empty() {
                continue;
            }

            dep_candidates.sort_by_key(|c| {
                format!(
                    "{}|{}",
                    c.manifest,
                    c.toml_path
                        .iter()
                        .map(std::string::String::as_str)
                        .collect::<Vec<_>>()
                        .join(".")
                )
            });

            let mut findings_by_key: BTreeMap<String, FindingRef> = BTreeMap::new();
            let mut seen_member_keys = BTreeSet::new();
            for cand in &dep_candidates {
                findings_by_key.insert(stable_finding_key(&cand.finding), cand.finding.clone());
                let member_key = format!(
                    "{}|{}",
                    cand.manifest,
                    cand.toml_path
                        .iter()
                        .map(std::string::String::as_str)
                        .collect::<Vec<_>>()
                        .join(".")
                );
                if seen_member_keys.insert(member_key) {
                    ops.push(Self::member_op(cand));
                }
            }

            if !Self::workspace_dep_exists(repo, &dep) {
                let findings = findings_by_key.into_values().collect();
                ops.push(Self::root_op(&dep, &selected_version, findings));
            }
        }

        Ok(ops)
    }
}

#[derive(Debug, Clone)]
struct RawCandidate {
    manifest: Utf8PathBuf,
    dep: String,
    selected_version: String,
    toml_path: Vec<String>,
    finding: FindingRef,
}

#[derive(Debug, Clone)]
struct ConsolidationCandidate {
    manifest: Utf8PathBuf,
    dep: String,
    selected_version: String,
    toml_path: Vec<String>,
    preserved: DepPreserve,
    finding: FindingRef,
}

#[derive(Debug, Clone, Default)]
struct DepPreserve {
    package: Option<String>,
    optional: Option<bool>,
    default_features: Option<bool>,
    features: Vec<String>,
}

impl DepPreserve {
    fn to_json(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut map = serde_json::Map::new();
        if let Some(pkg) = &self.package {
            map.insert(
                "package".to_string(),
                serde_json::Value::String(pkg.clone()),
            );
        }
        if let Some(optional) = self.optional {
            map.insert("optional".to_string(), serde_json::Value::Bool(optional));
        }
        if let Some(default_features) = self.default_features {
            map.insert(
                "default_features".to_string(),
                serde_json::Value::Bool(default_features),
            );
        }
        if !self.features.is_empty() {
            map.insert(
                "features".to_string(),
                serde_json::Value::Array(
                    self.features
                        .iter()
                        .map(|f| serde_json::Value::String(f.clone()))
                        .collect(),
                ),
            );
        }
        map
    }
}

fn parse_toml_path(v: &serde_json::Value) -> Option<Vec<String>> {
    let arr = v.as_array()?;
    let path: Vec<String> = arr
        .iter()
        .filter_map(|item| item.as_str().map(|s| s.to_string()))
        .collect();
    if path.len() < 2 {
        return None;
    }
    Some(path)
}

fn get_dep_item<'a>(doc: &'a DocumentMut, toml_path: &[String]) -> Option<&'a Item> {
    if toml_path.len() < 2 {
        return None;
    }

    if toml_path[0] == "target" {
        if toml_path.len() < 4 {
            return None;
        }
        let cfg = &toml_path[1];
        let table_name = &toml_path[2];
        let dep = &toml_path[3];

        let target = doc.get("target")?.as_table()?;
        let cfg_tbl = target.get(cfg)?.as_table()?;
        let deps = cfg_tbl.get(table_name)?.as_table()?;
        return deps.get(dep);
    }

    let table_name = &toml_path[0];
    let dep = &toml_path[1];
    doc.get(table_name)?.as_table()?.get(dep)
}

fn dep_preserve_from_item(item: &Item) -> Option<DepPreserve> {
    if item.as_value().and_then(|v| v.as_str()).is_some() {
        return Some(DepPreserve::default());
    }

    if let Some(inline) = item.as_inline_table() {
        let workspace_true = inline
            .get("workspace")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if workspace_true || inline.get("path").is_some() || inline.get("git").is_some() {
            return None;
        }

        return Some(DepPreserve {
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
        });
    }

    if let Some(tbl) = item.as_table() {
        let workspace_true = tbl
            .get("workspace")
            .and_then(|i| i.as_value())
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if workspace_true || tbl.get("path").is_some() || tbl.get("git").is_some() {
            return None;
        }

        return Some(DepPreserve {
            package: tbl
                .get("package")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            optional: tbl
                .get("optional")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_bool()),
            default_features: tbl
                .get("default-features")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_bool()),
            features: tbl
                .get("features")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        });
    }

    None
}

fn stable_finding_key(f: &FindingRef) -> String {
    let loc = f
        .path
        .as_ref()
        .map(|p| format!("{}:{}", p, f.line.unwrap_or(0)))
        .unwrap_or_else(|| "no_location".to_string());
    format!(
        "{}/{}/{}|{}",
        f.source,
        f.check_id.clone().unwrap_or_default(),
        f.code,
        loc
    )
}

fn fix_key_for(f: &FindingRef) -> String {
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
            self.files.contains_key(&self.key_for(rel))
        }
    }

    fn receipt_set_for(findings: Vec<Finding>) -> ReceiptSet {
        let receipt = ReceiptEnvelope {
            schema: "depguard.report.v1".to_string(),
            tool: ToolInfo {
                name: "depguard".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings,
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

    fn duplicate_finding(
        manifest_path: &str,
        dep: &str,
        selected_version: &str,
        toml_path: &[&str],
    ) -> Finding {
        Finding {
            severity: Default::default(),
            check_id: Some("deps.duplicate_dependency_versions".to_string()),
            code: Some("duplicate_version".to_string()),
            message: Some("duplicate dependency versions".to_string()),
            location: Some(Location {
                path: Utf8PathBuf::from(manifest_path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: Some(serde_json::json!({
                "dep": dep,
                "selected_version": selected_version,
                "toml_path": toml_path,
            })),
            ..Default::default()
        }
    }

    fn plan_ctx() -> PlanContext {
        PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        }
    }

    #[test]
    fn plan_emits_workspace_and_member_ops() {
        let repo = TestRepo::new(&[
            (
                "crates/a/Cargo.toml",
                r#"
                [package]
                name = "a"

                [dependencies]
                serde = "1.0.200"
                "#,
            ),
            (
                "crates/b/Cargo.toml",
                r#"
                [package]
                name = "b"

                [dependencies]
                serde = { version = "1.0.180", features = ["derive"] }
                "#,
            ),
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n",
            ),
        ]);

        let receipt_set = receipt_set_for(vec![
            duplicate_finding(
                "crates/a/Cargo.toml",
                "serde",
                "1.0.200",
                &["dependencies", "serde"],
            ),
            duplicate_finding(
                "crates/b/Cargo.toml",
                "serde",
                "1.0.200",
                &["dependencies", "serde"],
            ),
        ]);

        let ops = DuplicateDepsConsolidationFixer
            .plan(&plan_ctx(), &repo, &receipt_set)
            .expect("plan");

        assert_eq!(ops.len(), 3);
        assert!(ops.iter().any(|op| {
            op.target.path == "Cargo.toml"
                && matches!(
                    op.kind,
                    OpKind::TomlTransform {
                        ref rule_id,
                        args: Some(_)
                    } if rule_id == "ensure_workspace_dependency_version"
                )
        }));

        let member_ops: Vec<&PlanOp> = ops
            .iter()
            .filter(|op| {
                matches!(
                    op.kind,
                    OpKind::TomlTransform {
                        ref rule_id,
                        args: Some(_)
                    } if rule_id == "use_workspace_dependency"
                )
            })
            .collect();
        assert_eq!(member_ops.len(), 2);
        let with_features = member_ops
            .iter()
            .find(|op| op.target.path == "crates/b/Cargo.toml")
            .expect("crate b op");
        if let OpKind::TomlTransform {
            args: Some(args), ..
        } = &with_features.kind
        {
            assert_eq!(args["preserved"]["features"], serde_json::json!(["derive"]));
        } else {
            panic!("expected args");
        }
    }

    #[test]
    fn plan_skips_when_selected_versions_conflict() {
        let repo = TestRepo::new(&[
            (
                "crates/a/Cargo.toml",
                "[package]\nname = \"a\"\n[dependencies]\nserde = \"1.0.0\"\n",
            ),
            (
                "crates/b/Cargo.toml",
                "[package]\nname = \"b\"\n[dependencies]\nserde = \"1.1.0\"\n",
            ),
        ]);

        let receipt_set = receipt_set_for(vec![
            duplicate_finding(
                "crates/a/Cargo.toml",
                "serde",
                "1.1.0",
                &["dependencies", "serde"],
            ),
            duplicate_finding(
                "crates/b/Cargo.toml",
                "serde",
                "1.0.0",
                &["dependencies", "serde"],
            ),
        ]);

        let ops = DuplicateDepsConsolidationFixer
            .plan(&plan_ctx(), &repo, &receipt_set)
            .expect("plan");
        assert!(ops.is_empty());
    }

    #[test]
    fn plan_skips_when_required_data_is_missing() {
        let repo = TestRepo::new(&[(
            "crates/a/Cargo.toml",
            "[package]\nname = \"a\"\n[dependencies]\nserde = \"1.0.0\"\n",
        )]);
        let finding = Finding {
            severity: Default::default(),
            check_id: Some("deps.duplicate_dependency_versions".to_string()),
            code: Some("duplicate_version".to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from("crates/a/Cargo.toml"),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: Some(serde_json::json!({
                "dep": "serde",
                "selected_version": "1.0.0"
            })),
            ..Default::default()
        };
        let receipt_set = receipt_set_for(vec![finding]);

        let ops = DuplicateDepsConsolidationFixer
            .plan(&plan_ctx(), &repo, &receipt_set)
            .expect("plan");
        assert!(ops.is_empty());
    }

    #[test]
    fn plan_skips_root_op_when_workspace_dep_exists() {
        let repo = TestRepo::new(&[
            (
                "crates/a/Cargo.toml",
                r#"
                [package]
                name = "a"

                [dependencies]
                serde = "1.0.200"
                "#,
            ),
            (
                "crates/b/Cargo.toml",
                r#"
                [package]
                name = "b"

                [dependencies]
                serde = { version = "1.0.200", features = ["derive"] }
                "#,
            ),
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n\n[workspace.dependencies]\nserde = \"1.0.200\"\n",
            ),
        ]);

        let receipt_set = receipt_set_for(vec![
            duplicate_finding(
                "crates/a/Cargo.toml",
                "serde",
                "1.0.200",
                &["dependencies", "serde"],
            ),
            duplicate_finding(
                "crates/b/Cargo.toml",
                "serde",
                "1.0.200",
                &["dependencies", "serde"],
            ),
        ]);

        let ops = DuplicateDepsConsolidationFixer
            .plan(&plan_ctx(), &repo, &receipt_set)
            .expect("plan");

        // Should have only member ops, no root op
        assert_eq!(ops.len(), 2);
        assert!(
            !ops.iter().any(|op| {
                op.target.path == "Cargo.toml"
                    && matches!(
                        op.kind,
                        OpKind::TomlTransform {
                            ref rule_id,
                            args: Some(_)
                        } if rule_id == "ensure_workspace_dependency_version"
                    )
            }),
            "root op should not be emitted when workspace dep already exists"
        );

        // All ops should be member ops
        for op in &ops {
            if let OpKind::TomlTransform { ref rule_id, .. } = op.kind {
                assert_eq!(rule_id, "use_workspace_dependency");
            }
        }
    }

    #[test]
    fn dep_preserve_skips_path_dependency() {
        let doc = r#"
            [dependencies]
            local = { path = "../local", version = "0.1.0" }
        "#
        .parse::<DocumentMut>()
        .expect("parse");

        let item =
            get_dep_item(&doc, &["dependencies".to_string(), "local".to_string()]).expect("item");
        assert!(dep_preserve_from_item(item).is_none());
    }
}
