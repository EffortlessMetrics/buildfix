use crate::fixers::{Fixer, FixerMeta};
use crate::planner::{MatchedFinding, ReceiptSet};
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{FindingRef, PlanOp, Rationale};
use buildfix_types::receipt::{AnalysisDepth, FindingContext};
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

    fn manifest_paths_from_triggers(triggers: &[MatchedFinding]) -> BTreeSet<Utf8PathBuf> {
        let mut out = BTreeSet::new();
        for t in triggers {
            let Some(path) = &t.finding.path else { continue };
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
        let matched = receipts.matching_findings_with_data(Self::SENSORS, Self::CHECK_IDS, &[]);
        if matched.is_empty() {
            return Ok(vec![]);
        }

        let edition = Self::canonical_edition(repo);

        // Group findings by manifest, collecting evidence for safety classification
        let mut triggers_by_manifest: BTreeMap<Utf8PathBuf, Vec<MatchedFinding>> = BTreeMap::new();
        for m in &matched {
            if let Some(path) = &m.finding.path {
                triggers_by_manifest
                    .entry(Utf8PathBuf::from(path.clone()))
                    .or_default()
                    .push(m.clone());
            }
        }

        let mut fixes = Vec::new();
        for manifest in Self::manifest_paths_from_triggers(&matched) {
            let contents = match repo.read_to_string(&manifest) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Some(ed) = &edition
                && !Self::needs_change(&contents, ed)
            {
                continue;
            }

            // Collect findings and evidence for this manifest
            let manifest_findings = triggers_by_manifest
                .get(&manifest)
                .cloned()
                .unwrap_or_default();

            // Determine safety class based on evidence
            let (safety, params_required, edition_value) = match &edition {
                Some(ed) => {
                    // Check for evidence-based safety promotion
                    let evidence = aggregate_evidence(&manifest_findings);
                    let safety = determine_safety_class(&evidence, true);
                    (safety, vec![], serde_json::Value::String(ed.clone()))
                }
                None => (
                    SafetyClass::Unsafe,
                    vec!["edition".to_string()],
                    serde_json::Value::Null,
                ),
            };

            let mut args = serde_json::Map::new();
            args.insert("edition".to_string(), edition_value);

            let findings: Vec<FindingRef> = manifest_findings
                .iter()
                .map(|m| m.finding.clone())
                .collect();
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

/// Aggregated evidence from findings for safety classification.
#[derive(Default)]
struct Evidence {
    confidence: Option<f64>,
    context: Option<FindingContext>,
}

/// Aggregates evidence from multiple matched findings.
fn aggregate_evidence(findings: &[MatchedFinding]) -> Evidence {
    // Take the first finding's evidence (they should all be consistent)
    findings.first().map_or(Evidence::default(), |f| Evidence {
        confidence: f.confidence,
        context: f.context.clone(),
    })
}

/// Determines the safety class for edition normalization based on evidence.
///
/// # Safety Promotion Logic
///
/// An operation is promoted to [`SafetyClass::Safe`] when ALL of the following conditions are met:
/// - **Workspace canonical exists**: The workspace has a defined edition in `[workspace.package]`
/// - **Full consensus**: All workspace crates agree on the edition value
/// - **High confidence** (≥0.9): The sensor reports high certainty
///
/// Without full consensus but with workspace canonical, the operation is [`SafetyClass::Guarded`].
///
/// Without workspace canonical, the operation is [`SafetyClass::Unsafe`] (requires user input).
fn determine_safety_class(evidence: &Evidence, has_workspace_canonical: bool) -> SafetyClass {
    // If we have a workspace canonical, check for promotion to Safe
    if has_workspace_canonical {
        // Check for full consensus + high confidence → Safe
        if has_full_consensus(&evidence.context) && is_high_confidence(evidence) {
            return SafetyClass::Safe;
        }
        // Workspace canonical exists but not full consensus → Guarded
        return SafetyClass::Guarded;
    }

    // No workspace canonical - requires user input
    SafetyClass::Unsafe
}

/// Checks if all workspace crates agree on the value (full consensus).
fn has_full_consensus(context: &Option<FindingContext>) -> bool {
    context
        .as_ref()
        .and_then(|ctx| ctx.workspace.as_ref())
        .is_some_and(|ws| ws.all_crates_agree)
}

/// Checks if the evidence has high confidence (≥0.9).
fn is_high_confidence(evidence: &Evidence) -> bool {
    evidence.confidence.is_some_and(|c| c >= 0.9)
}

/// Checks if the analysis depth is full or deep.
#[allow(dead_code)]
fn has_full_analysis_depth(context: &Option<FindingContext>) -> bool {
    context
        .as_ref()
        .and_then(|ctx| ctx.analysis_depth)
        .is_some_and(|depth| matches!(depth, AnalysisDepth::Full | AnalysisDepth::Deep))
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
    use buildfix_types::receipt::{
        Finding, FindingContext, Location, ReceiptEnvelope, RunInfo, ToolInfo, Verdict,
        WorkspaceContext,
    };
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
                check_id: Some("edition.consistent".to_string()),
                code: Some("EDITION".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from(path),
                    line: Some(1),
                    column: None,
                }),
                fingerprint: None,
                data: None,
                ..Default::default()
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
    fn canonical_edition_prefers_workspace_package() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [workspace.package]
                edition = "2021"

                [package]
                edition = "2018"
            "#,
        )]);
        let edition = EditionUpgradeFixer::canonical_edition(&repo);
        assert_eq!(edition.as_deref(), Some("2021"));
    }

    #[test]
    fn canonical_edition_falls_back_to_package() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [package]
                edition = "2018"
            "#,
        )]);
        let edition = EditionUpgradeFixer::canonical_edition(&repo);
        assert_eq!(edition.as_deref(), Some("2018"));
    }

    #[test]
    fn needs_change_detects_mismatch() {
        let manifest = r#"
            [package]
            edition = "2018"
        "#;
        assert!(EditionUpgradeFixer::needs_change("not toml", "2021"));
        assert!(EditionUpgradeFixer::needs_change("[workspace]", "2021"));
        assert!(EditionUpgradeFixer::needs_change(manifest, "2021"));
        assert!(!EditionUpgradeFixer::needs_change(manifest, "2018"));
    }

    #[test]
    fn plan_emits_guarded_fix_with_canonical_edition() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.package]
                    edition = "2021"
                "#,
            ),
            (
                "crates/a/Cargo.toml",
                r#"
                    [package]
                    name = "a"
                    edition = "2018"
                "#,
            ),
        ]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        let receipt_set = receipt_set_for("crates/a/Cargo.toml");
        let fixes = EditionUpgradeFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        let op = &fixes[0];
        assert_eq!(op.safety, SafetyClass::Guarded);
        assert!(op.params_required.is_empty());
        match &op.kind {
            OpKind::TomlTransform { rule_id, args } => {
                assert_eq!(rule_id, "set_package_edition");
                assert_eq!(args.as_ref().unwrap()["edition"], "2021");
            }
            _ => panic!("expected toml transform"),
        }
    }

    #[test]
    fn plan_emits_unsafe_fix_without_canonical_edition() {
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
        let fixes = EditionUpgradeFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        let op = &fixes[0];
        assert_eq!(op.safety, SafetyClass::Unsafe);
        assert_eq!(op.params_required, vec!["edition".to_string()]);
    }

    // =========================================================================
    // Evidence-based safety classification tests
    // =========================================================================

    fn receipt_set_with_evidence(
        path: &str,
        confidence: Option<f64>,
        context: Option<FindingContext>,
    ) -> ReceiptSet {
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
                check_id: Some("edition.consistent".to_string()),
                code: Some("EDITION".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from(path),
                    line: Some(1),
                    column: None,
                }),
                fingerprint: None,
                data: None,
                confidence,
                provenance: None,
                context,
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
    fn determine_safety_class_promotes_to_safe_with_full_evidence() {
        // Full consensus + high confidence + workspace canonical → Safe
        let evidence = Evidence {
            confidence: Some(0.95),
            context: Some(FindingContext {
                workspace: Some(WorkspaceContext {
                    all_crates_agree: true,
                    ..Default::default()
                }),
                analysis_depth: None,
            }),
        };

        assert_eq!(determine_safety_class(&evidence, true), SafetyClass::Safe);
    }

    #[test]
    fn determine_safety_class_guarded_with_workspace_canonical_no_consensus() {
        // Workspace canonical exists but no full consensus → Guarded
        let evidence = Evidence {
            confidence: Some(0.95),
            context: Some(FindingContext {
                workspace: Some(WorkspaceContext {
                    all_crates_agree: false,
                    ..Default::default()
                }),
                analysis_depth: None,
            }),
        };

        assert_eq!(
            determine_safety_class(&evidence, true),
            SafetyClass::Guarded
        );
    }

    #[test]
    fn determine_safety_class_guarded_with_consensus_low_confidence() {
        // Full consensus but low confidence → Guarded (not Safe)
        let evidence = Evidence {
            confidence: Some(0.7), // Below 0.9 threshold
            context: Some(FindingContext {
                workspace: Some(WorkspaceContext {
                    all_crates_agree: true,
                    ..Default::default()
                }),
                analysis_depth: None,
            }),
        };

        assert_eq!(
            determine_safety_class(&evidence, true),
            SafetyClass::Guarded
        );
    }

    #[test]
    fn determine_safety_class_unsafe_without_workspace_canonical() {
        // No workspace canonical → Unsafe (requires user input)
        let evidence = Evidence {
            confidence: Some(0.95),
            context: Some(FindingContext {
                workspace: Some(WorkspaceContext {
                    all_crates_agree: true,
                    ..Default::default()
                }),
                analysis_depth: None,
            }),
        };

        assert_eq!(
            determine_safety_class(&evidence, false),
            SafetyClass::Unsafe
        );
    }

    #[test]
    fn determine_safety_class_unsafe_with_no_evidence() {
        // No evidence at all → Unsafe without workspace canonical
        let evidence = Evidence::default();
        assert_eq!(
            determine_safety_class(&evidence, false),
            SafetyClass::Unsafe
        );
    }

    #[test]
    fn plan_promotes_to_safe_with_full_evidence() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.package]
                    edition = "2021"
                "#,
            ),
            (
                "crates/a/Cargo.toml",
                r#"
                    [package]
                    name = "a"
                    edition = "2018"
                "#,
            ),
        ]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        // High confidence + full consensus → Safe
        let receipt_set = receipt_set_with_evidence(
            "crates/a/Cargo.toml",
            Some(0.95),
            Some(FindingContext {
                workspace: Some(WorkspaceContext {
                    all_crates_agree: true,
                    ..Default::default()
                }),
                analysis_depth: None,
            }),
        );

        let fixes = EditionUpgradeFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].safety, SafetyClass::Safe);
    }

    #[test]
    fn plan_remains_guarded_with_partial_evidence() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.package]
                    edition = "2021"
                "#,
            ),
            (
                "crates/a/Cargo.toml",
                r#"
                    [package]
                    name = "a"
                    edition = "2018"
                "#,
            ),
        ]);

        let ctx = PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        };

        // Low confidence → Guarded (not promoted to Safe)
        let receipt_set = receipt_set_with_evidence(
            "crates/a/Cargo.toml",
            Some(0.7), // Below 0.9 threshold
            Some(FindingContext {
                workspace: Some(WorkspaceContext {
                    all_crates_agree: true,
                    ..Default::default()
                }),
                analysis_depth: None,
            }),
        );

        let fixes = EditionUpgradeFixer
            .plan(&ctx, &repo, &receipt_set)
            .expect("plan");
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].safety, SafetyClass::Guarded);
    }
}
