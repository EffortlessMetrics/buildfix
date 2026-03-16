use crate::fixers::{Fixer, FixerMeta};
use crate::planner::{MatchedFinding, ReceiptSet};
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{FindingRef, PlanOp, Rationale};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::BTreeMap;
use toml_edit::{DocumentMut, Item};

pub struct RemoveUnusedDepsFixer;

impl RemoveUnusedDepsFixer {
    const FIX_ID: &'static str = "cargo.remove_unused_deps";
    const DESCRIPTION: &'static str = "Removes dependency entries reported as unused";
    const SENSORS: &'static [&'static str] = &["cargo-udeps", "udeps", "cargo-machete", "machete"];
    const CHECK_IDS: &'static [&'static str] = &[
        "deps.unused_dependency",
        "deps.unused_dependencies",
        "cargo.unused_dependency",
        "cargo.unused_dependencies",
        "udeps.unused_dependency",
        "machete.unused_dependency",
    ];

    fn parse_candidate(matched: &MatchedFinding) -> Option<RemoveCandidate> {
        let manifest_path = matched.finding.path.as_ref()?;
        if !manifest_path.ends_with("Cargo.toml") {
            return None;
        }

        let data = matched.data.as_ref()?.as_object()?;
        let toml_path = Self::extract_toml_path(data)?;
        if !is_valid_dep_toml_path(&toml_path) {
            return None;
        }

        Some(RemoveCandidate {
            manifest: Utf8PathBuf::from(manifest_path.clone()),
            toml_path,
            finding: matched.finding.clone(),
        })
    }

    fn extract_toml_path(data: &serde_json::Map<String, serde_json::Value>) -> Option<Vec<String>> {
        if let Some(path) = data.get("toml_path").and_then(parse_toml_path) {
            return Some(path);
        }

        let dep = data
            .get("dep")
            .or_else(|| data.get("dependency"))
            .or_else(|| data.get("crate"))
            .or_else(|| data.get("name"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())?;

        let table = data
            .get("table")
            .or_else(|| data.get("dep_table"))
            .or_else(|| data.get("dependency_table"))
            .or_else(|| data.get("section"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())?;

        if let Some(target_cfg) = data
            .get("target")
            .or_else(|| data.get("target_cfg"))
            .or_else(|| data.get("cfg"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(vec![
                "target".to_string(),
                target_cfg.to_string(),
                table.to_string(),
                dep.to_string(),
            ]);
        }

        Some(vec![table.to_string(), dep.to_string()])
    }

    fn dep_item_exists(repo: &dyn RepoView, manifest: &Utf8Path, toml_path: &[String]) -> bool {
        let Ok(contents) = repo.read_to_string(manifest) else {
            return false;
        };
        let Ok(doc) = contents.parse::<DocumentMut>() else {
            return false;
        };
        get_dep_item(&doc, toml_path).is_some()
    }
}

impl Fixer for RemoveUnusedDepsFixer {
    fn meta(&self) -> FixerMeta {
        FixerMeta {
            fix_key: Self::FIX_ID,
            description: Self::DESCRIPTION,
            safety: SafetyClass::Unsafe,
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

        #[derive(Debug, Clone)]
        struct Group {
            manifest: Utf8PathBuf,
            toml_path: Vec<String>,
            findings: BTreeMap<String, FindingRef>,
        }

        let mut grouped: BTreeMap<String, Group> = BTreeMap::new();

        for m in &matched {
            let Some(candidate) = Self::parse_candidate(m) else {
                continue;
            };
            if !Self::dep_item_exists(repo, &candidate.manifest, &candidate.toml_path) {
                continue;
            }

            let candidate_key = format!("{}|{}", candidate.manifest, candidate.toml_path.join("."));

            let entry = grouped.entry(candidate_key).or_insert_with(|| Group {
                manifest: candidate.manifest.clone(),
                toml_path: candidate.toml_path.clone(),
                findings: BTreeMap::new(),
            });

            entry.findings.insert(
                stable_finding_key(&candidate.finding),
                candidate.finding.clone(),
            );
        }

        let mut ops = Vec::new();
        for (_k, group) in grouped {
            let findings: Vec<FindingRef> = group.findings.into_values().collect();
            let fix_key = findings
                .first()
                .map(fix_key_for)
                .unwrap_or_else(|| "unknown/-/-".to_string());

            ops.push(PlanOp {
                id: String::new(),
                safety: SafetyClass::Unsafe,
                blocked: false,
                blocked_reason: None,
                blocked_reason_token: None,
                target: OpTarget {
                    path: group.manifest.to_string(),
                },
                kind: OpKind::TomlRemove {
                    toml_path: group.toml_path,
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

        Ok(ops)
    }
}

#[derive(Debug, Clone)]
struct RemoveCandidate {
    manifest: Utf8PathBuf,
    toml_path: Vec<String>,
    finding: FindingRef,
}

fn parse_toml_path(v: &serde_json::Value) -> Option<Vec<String>> {
    let arr = v.as_array()?;
    let path: Vec<String> = arr
        .iter()
        .map(|item| item.as_str().map(str::trim))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    if path.is_empty() {
        return None;
    }
    Some(path)
}

fn is_dep_table(table_name: &str) -> bool {
    matches!(
        table_name,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    )
}

fn is_valid_dep_toml_path(path: &[String]) -> bool {
    match path {
        [table, dep] => is_dep_table(table) && !dep.trim().is_empty(),
        [target, cfg, table, dep] => {
            target == "target"
                && !cfg.trim().is_empty()
                && is_dep_table(table)
                && !dep.trim().is_empty()
        }
        _ => false,
    }
}

fn get_dep_item<'a>(doc: &'a DocumentMut, toml_path: &[String]) -> Option<&'a Item> {
    if !is_valid_dep_toml_path(toml_path) {
        return None;
    }

    if toml_path[0] == "target" {
        let target = doc.get("target")?.as_table()?;
        let cfg_tbl = target.get(&toml_path[1])?.as_table()?;
        let deps = cfg_tbl.get(&toml_path[2])?.as_table()?;
        return deps.get(&toml_path[3]);
    }

    let deps = doc.get(&toml_path[0])?.as_table()?;
    deps.get(&toml_path[1])
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

    fn make_receipt_set(findings: Vec<Finding>) -> ReceiptSet {
        let receipt = ReceiptEnvelope {
            schema: "cargo-machete.report.v1".to_string(),
            tool: ToolInfo {
                name: "cargo-machete".to_string(),
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
            path: Utf8PathBuf::from("artifacts/cargo-machete/report.json"),
            sensor_id: "cargo-machete".to_string(),
            receipt: Ok(receipt),
        }];
        ReceiptSet::from_loaded(&loaded)
    }

    fn make_finding(path: &str, check_id: &str, code: &str, data: serde_json::Value) -> Finding {
        Finding {
            severity: Default::default(),
            check_id: Some(check_id.to_string()),
            code: Some(code.to_string()),
            message: None,
            location: Some(Location {
                path: Utf8PathBuf::from(path),
                line: Some(1),
                column: None,
            }),
            fingerprint: None,
            data: Some(data),
        }
    }

    fn ctx() -> PlanContext {
        PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        }
    }

    #[test]
    fn plan_emits_unsafe_toml_remove_for_unused_dependency() {
        let repo = TestRepo::new(&[(
            "crates/a/Cargo.toml",
            r#"
                [package]
                name = "a"

                [dependencies]
                serde = "1.0"
            "#,
        )]);

        let receipts = make_receipt_set(vec![make_finding(
            "crates/a/Cargo.toml",
            "deps.unused_dependency",
            "unused_dep",
            serde_json::json!({
                "toml_path": ["dependencies", "serde"],
                "dep": "serde"
            }),
        )]);

        let ops = RemoveUnusedDepsFixer
            .plan(&ctx(), &repo, &receipts)
            .expect("plan");
        assert_eq!(ops.len(), 1);
        let op = &ops[0];
        assert_eq!(op.safety, SafetyClass::Unsafe);
        assert_eq!(op.target.path, "crates/a/Cargo.toml");
        assert!(matches!(
            op.kind,
            OpKind::TomlRemove { ref toml_path } if toml_path == &vec!["dependencies".to_string(), "serde".to_string()]
        ));
    }

    #[test]
    fn plan_builds_path_from_table_and_dependency_fields() {
        let repo = TestRepo::new(&[(
            "crates/a/Cargo.toml",
            r#"
                [package]
                name = "a"

                [dev-dependencies]
                tempfile = "3"
            "#,
        )]);

        let receipts = make_receipt_set(vec![make_finding(
            "crates/a/Cargo.toml",
            "deps.unused_dependency",
            "unused_dep",
            serde_json::json!({
                "table": "dev-dependencies",
                "dependency": "tempfile"
            }),
        )]);

        let ops = RemoveUnusedDepsFixer
            .plan(&ctx(), &repo, &receipts)
            .expect("plan");
        assert_eq!(ops.len(), 1);
        assert!(matches!(
            ops[0].kind,
            OpKind::TomlRemove { ref toml_path } if toml_path == &vec!["dev-dependencies".to_string(), "tempfile".to_string()]
        ));
    }

    #[test]
    fn plan_skips_invalid_paths_and_missing_dependencies() {
        let repo = TestRepo::new(&[(
            "crates/a/Cargo.toml",
            r#"
                [package]
                name = "a"

                [dependencies]
                serde = "1.0"
            "#,
        )]);

        let invalid_path = make_finding(
            "crates/a/Cargo.toml",
            "deps.unused_dependency",
            "unused_dep",
            serde_json::json!({
                "toml_path": ["package", "name"]
            }),
        );
        let missing_dep = make_finding(
            "crates/a/Cargo.toml",
            "deps.unused_dependency",
            "unused_dep",
            serde_json::json!({
                "toml_path": ["dependencies", "tokio"]
            }),
        );

        let receipts = make_receipt_set(vec![invalid_path, missing_dep]);
        let ops = RemoveUnusedDepsFixer
            .plan(&ctx(), &repo, &receipts)
            .expect("plan");
        assert!(ops.is_empty());
    }

    #[test]
    fn plan_deduplicates_ops_for_same_manifest_and_toml_path() {
        let repo = TestRepo::new(&[(
            "crates/a/Cargo.toml",
            r#"
                [package]
                name = "a"

                [dependencies]
                serde = "1.0"
            "#,
        )]);

        let receipts = make_receipt_set(vec![
            make_finding(
                "crates/a/Cargo.toml",
                "deps.unused_dependency",
                "unused_dep",
                serde_json::json!({
                    "toml_path": ["dependencies", "serde"]
                }),
            ),
            make_finding(
                "crates/a/Cargo.toml",
                "deps.unused_dependencies",
                "unused_dep",
                serde_json::json!({
                    "toml_path": ["dependencies", "serde"]
                }),
            ),
        ]);

        let ops = RemoveUnusedDepsFixer
            .plan(&ctx(), &repo, &receipts)
            .expect("plan");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].rationale.findings.len(), 2);
    }

    #[test]
    fn is_valid_dep_toml_path_supports_target_tables() {
        assert!(is_valid_dep_toml_path(&[
            "target".to_string(),
            "cfg(windows)".to_string(),
            "dependencies".to_string(),
            "winapi".to_string()
        ]));
        assert!(!is_valid_dep_toml_path(&[
            "target".to_string(),
            "cfg(windows)".to_string(),
            "package".to_string(),
            "name".to_string()
        ]));
    }
}
