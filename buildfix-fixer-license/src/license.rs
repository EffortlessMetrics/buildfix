use crate::fixers::{Fixer, FixerMeta};
use crate::planner::{MatchedFinding, ReceiptSet};
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{FindingRef, PlanOp, Rationale};
use buildfix_types::receipt::{AnalysisDepth, FindingContext};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::DocumentMut;

pub struct LicenseNormalizeFixer;

impl LicenseNormalizeFixer {
    const FIX_ID: &'static str = "cargo.normalize_license";
    const DESCRIPTION: &'static str =
        "Normalizes per-crate package.license to workspace canonical license";
    const SENSORS: &'static [&'static str] = &["cargo-deny", "deny"];
    const CHECK_IDS: &'static [&'static str] = &[
        "licenses.unlicensed",
        "licenses.missing",
        "licenses.missing_license",
        "license.unlicensed",
        "license.missing",
        "license.missing_license",
        "cargo.licenses.unlicensed",
        "cargo.licenses.missing_license",
    ];

    fn canonical_license(repo: &dyn RepoView) -> Option<String> {
        let contents = repo.read_to_string(Utf8Path::new("Cargo.toml")).ok()?;
        let doc = contents.parse::<DocumentMut>().ok()?;

        // Preferred: [workspace.package].license
        if let Some(ws) = doc.get("workspace").and_then(|i| i.as_table())
            && let Some(pkg) = ws.get("package").and_then(|i| i.as_table())
            && let Some(license) = pkg
                .get("license")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
        {
            return Some(license.to_string());
        }

        // Fallback: [package].license (root package in a workspace).
        if let Some(pkg) = doc.get("package").and_then(|i| i.as_table())
            && let Some(license) = pkg
                .get("license")
                .and_then(|i| i.as_value())
                .and_then(|v| v.as_str())
        {
            return Some(license.to_string());
        }

        None
    }

    fn manifest_from_match(repo: &dyn RepoView, matched: &MatchedFinding) -> Option<Utf8PathBuf> {
        if let Some(path) = matched
            .data
            .as_ref()
            .and_then(|v| v.as_object())
            .and_then(Self::extract_manifest_from_data)
            && let Some(resolved) = Self::resolve_manifest_path(repo, Utf8Path::new(path))
        {
            return Some(resolved);
        }

        let location = matched.finding.path.as_deref()?;
        Self::resolve_manifest_path(repo, Utf8Path::new(location))
    }

    fn extract_manifest_from_data(
        data: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<&str> {
        for key in [
            "manifest_path",
            "manifest",
            "manifestPath",
            "cargo_toml",
            "cargo_toml_path",
            "path",
        ] {
            if let Some(path) = data.get(key).and_then(|v| v.as_str())
                && !path.trim().is_empty()
            {
                return Some(path);
            }
        }
        None
    }

    fn resolve_manifest_path(repo: &dyn RepoView, path: &Utf8Path) -> Option<Utf8PathBuf> {
        let normalized = Self::normalize_repo_path(repo, path);
        if normalized.file_name() == Some("Cargo.toml") {
            return Some(Self::normalize_utf8_path(&normalized));
        }

        // If location points to a file, try its parent first.
        let mut cursor = if normalized.extension().is_some() {
            normalized.parent().map(|p| p.to_path_buf())?
        } else {
            normalized
        };

        // Walk up toward repo root and look for Cargo.toml.
        loop {
            let candidate = cursor.join("Cargo.toml");
            if repo.exists(&candidate) {
                return Some(Self::normalize_utf8_path(&candidate));
            }

            let Some(parent) = cursor.parent() else {
                break;
            };

            // Stop once we're at or above root.
            if parent == cursor {
                break;
            }
            cursor = parent.to_path_buf();
        }

        None
    }

    fn normalize_repo_path(repo: &dyn RepoView, path: &Utf8Path) -> Utf8PathBuf {
        if path.is_absolute() {
            if let Ok(stripped) = path.strip_prefix(repo.root()) {
                return Self::normalize_utf8_path(stripped);
            }
            return Self::normalize_utf8_path(path);
        }
        Self::normalize_utf8_path(path)
    }

    fn normalize_utf8_path(path: &Utf8Path) -> Utf8PathBuf {
        let raw = path.as_str().replace('\\', "/");
        let trimmed = raw.trim_start_matches("./");
        Utf8PathBuf::from(trimmed)
    }

    fn needs_change(contents: &str, license: &str) -> bool {
        let Ok(doc) = contents.parse::<DocumentMut>() else {
            return true;
        };
        let Some(pkg) = doc.get("package").and_then(|i| i.as_table()) else {
            return true;
        };

        let current = pkg
            .get("license")
            .and_then(|i| i.as_value())
            .and_then(|v| v.as_str());
        current != Some(license)
    }
}

impl Fixer for LicenseNormalizeFixer {
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

        let canonical = Self::canonical_license(repo);

        /// Groups findings by manifest, tracking evidence for safety classification.
        struct ManifestGroup {
            findings: BTreeMap<String, FindingRef>,
            /// Evidence from the first finding (used for safety classification).
            confidence: Option<f64>,
            context: Option<FindingContext>,
        }

        let mut triggers_by_manifest: BTreeMap<Utf8PathBuf, ManifestGroup> = BTreeMap::new();
        for m in &matched {
            let Some(manifest) = Self::manifest_from_match(repo, m) else {
                continue;
            };
            if !repo.exists(&manifest) {
                continue;
            }

            let entry = triggers_by_manifest.entry(manifest).or_insert_with(|| {
                ManifestGroup {
                    findings: BTreeMap::new(),
                    confidence: m.confidence,
                    context: m.context.clone(),
                }
            });

            entry
                .findings
                .insert(stable_finding_key(&m.finding), m.finding.clone());
        }

        let mut manifests = BTreeSet::new();
        manifests.extend(triggers_by_manifest.keys().cloned());

        let mut fixes = Vec::new();
        for manifest in manifests {
            let contents = match repo.read_to_string(&manifest) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Some(canonical_license) = &canonical
                && !Self::needs_change(&contents, canonical_license)
            {
                continue;
            }

            let Some(group) = triggers_by_manifest.get(&manifest) else {
                continue;
            };

            // Determine safety class based on evidence
            let (safety, params_required, license_value) = match &canonical {
                Some(license) => {
                    // Check for evidence-based safety promotion
                    let evidence = Evidence {
                        confidence: group.confidence,
                        context: group.context.clone(),
                    };
                    let safety = determine_safety_class(&evidence, true);
                    (safety, Vec::new(), serde_json::Value::String(license.clone()))
                }
                None => (
                    SafetyClass::Unsafe,
                    vec!["license".to_string()],
                    serde_json::Value::Null,
                ),
            };

            let mut args = serde_json::Map::new();
            args.insert("license".to_string(), license_value);

            let findings: Vec<FindingRef> = group.findings.values().cloned().collect();
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
                    rule_id: "set_package_license".to_string(),
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
struct Evidence {
    confidence: Option<f64>,
    context: Option<FindingContext>,
}

/// Determines the safety class for license normalization based on evidence.
///
/// # Safety Promotion Logic
///
/// An operation is promoted to [`SafetyClass::Safe`] when ALL of the following conditions are met:
/// - **Workspace canonical exists**: The workspace has a defined license in `[workspace.package]`
/// - **Full consensus**: All workspace crates agree on the license value
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
    use buildfix_types::receipt::{
        Finding, FindingContext, Location, ReceiptEnvelope, RunInfo, ToolInfo, Verdict,
        WorkspaceContext,
    };
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
                rel.strip_prefix(&self.root).unwrap_or(rel).to_string()
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
            self.files.contains_key(&self.key_for(rel))
        }
    }

    fn make_receipt_set(path: &str, data: Option<serde_json::Value>) -> ReceiptSet {
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: ToolInfo {
                name: "cargo-deny".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("licenses.unlicensed".to_string()),
                code: Some("missing_license".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from(path),
                    line: Some(1),
                    column: None,
                }),
                fingerprint: None,
                data,
                ..Default::default()
            }],
            capabilities: None,
            data: None,
        };

        let loaded = vec![LoadedReceipt {
            path: Utf8PathBuf::from("artifacts/cargo-deny/report.json"),
            sensor_id: "cargo-deny".to_string(),
            receipt: Ok(receipt),
        }];

        ReceiptSet::from_loaded(&loaded)
    }

    fn ctx() -> PlanContext {
        PlanContext {
            repo_root: Utf8PathBuf::from("."),
            artifacts_dir: Utf8PathBuf::from("artifacts"),
            config: PlannerConfig::default(),
        }
    }

    #[test]
    fn canonical_license_prefers_workspace_package() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [workspace.package]
                license = "Apache-2.0"

                [package]
                license = "MIT"
            "#,
        )]);

        let license = LicenseNormalizeFixer::canonical_license(&repo);
        assert_eq!(license.as_deref(), Some("Apache-2.0"));
    }

    #[test]
    fn canonical_license_falls_back_to_package() {
        let repo = TestRepo::new(&[(
            "Cargo.toml",
            r#"
                [package]
                license = "MIT"
            "#,
        )]);

        let license = LicenseNormalizeFixer::canonical_license(&repo);
        assert_eq!(license.as_deref(), Some("MIT"));
    }

    #[test]
    fn plan_emits_guarded_fix_with_canonical_license() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.package]
                    license = "MIT OR Apache-2.0"
                "#,
            ),
            (
                "crates/a/Cargo.toml",
                r#"
                    [package]
                    name = "a"
                    license = "MIT"
                "#,
            ),
            ("crates/a/src/lib.rs", ""),
        ]);

        let receipts = make_receipt_set("crates/a/src/lib.rs", None);
        let ops = LicenseNormalizeFixer
            .plan(&ctx(), &repo, &receipts)
            .expect("plan");

        assert_eq!(ops.len(), 1);
        let op = &ops[0];
        assert_eq!(op.safety, SafetyClass::Guarded);
        assert_eq!(op.target.path, "crates/a/Cargo.toml");
        assert!(op.params_required.is_empty());
        match &op.kind {
            OpKind::TomlTransform { rule_id, args } => {
                assert_eq!(rule_id, "set_package_license");
                assert_eq!(args.as_ref().expect("args")["license"], "MIT OR Apache-2.0");
            }
            _ => panic!("expected toml_transform"),
        }
    }

    #[test]
    fn plan_emits_unsafe_fix_without_canonical_license() {
        let repo = TestRepo::new(&[(
            "crates/a/Cargo.toml",
            r#"
                [package]
                name = "a"
            "#,
        )]);

        let receipts = make_receipt_set(
            "crates/a/src/lib.rs",
            Some(serde_json::json!({
                "manifest_path": "crates/a/Cargo.toml"
            })),
        );
        let ops = LicenseNormalizeFixer
            .plan(&ctx(), &repo, &receipts)
            .expect("plan");

        assert_eq!(ops.len(), 1);
        let op = &ops[0];
        assert_eq!(op.safety, SafetyClass::Unsafe);
        assert_eq!(op.params_required, vec!["license".to_string()]);
    }

    #[test]
    fn plan_skips_when_license_already_matches() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.package]
                    license = "MIT"
                "#,
            ),
            (
                "crates/a/Cargo.toml",
                r#"
                    [package]
                    name = "a"
                    license = "MIT"
                "#,
            ),
        ]);

        let receipts = make_receipt_set(
            "ignored.rs",
            Some(serde_json::json!({
                "manifest_path": "crates/a/Cargo.toml"
            })),
        );
        let ops = LicenseNormalizeFixer
            .plan(&ctx(), &repo, &receipts)
            .expect("plan");
        assert!(ops.is_empty());
    }

    // =========================================================================
    // Evidence-based safety classification tests
    // =========================================================================

    fn make_receipt_set_with_evidence(
        path: &str,
        data: Option<serde_json::Value>,
        confidence: Option<f64>,
        context: Option<FindingContext>,
    ) -> ReceiptSet {
        let receipt = ReceiptEnvelope {
            schema: "sensor.report.v1".to_string(),
            tool: ToolInfo {
                name: "cargo-deny".to_string(),
                version: None,
                repo: None,
                commit: None,
            },
            run: RunInfo::default(),
            verdict: Verdict::default(),
            findings: vec![Finding {
                severity: Default::default(),
                check_id: Some("licenses.unlicensed".to_string()),
                code: Some("missing_license".to_string()),
                message: None,
                location: Some(Location {
                    path: Utf8PathBuf::from(path),
                    line: Some(1),
                    column: None,
                }),
                fingerprint: None,
                data,
                confidence,
                provenance: None,
                context,
            }],
            capabilities: None,
            data: None,
        };

        let loaded = vec![LoadedReceipt {
            path: Utf8PathBuf::from("artifacts/cargo-deny/report.json"),
            sensor_id: "cargo-deny".to_string(),
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
        let evidence = Evidence {
            confidence: None,
            context: None,
        };
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
                    license = "MIT OR Apache-2.0"
                "#,
            ),
            (
                "crates/a/Cargo.toml",
                r#"
                    [package]
                    name = "a"
                    license = "MIT"
                "#,
            ),
            ("crates/a/src/lib.rs", ""),
        ]);

        // High confidence + full consensus → Safe
        let receipts = make_receipt_set_with_evidence(
            "crates/a/src/lib.rs",
            None,
            Some(0.95),
            Some(FindingContext {
                workspace: Some(WorkspaceContext {
                    all_crates_agree: true,
                    ..Default::default()
                }),
                analysis_depth: None,
            }),
        );

        let ops = LicenseNormalizeFixer
            .plan(&ctx(), &repo, &receipts)
            .expect("plan");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].safety, SafetyClass::Safe);
    }

    #[test]
    fn plan_remains_guarded_with_partial_evidence() {
        let repo = TestRepo::new(&[
            (
                "Cargo.toml",
                r#"
                    [workspace.package]
                    license = "MIT OR Apache-2.0"
                "#,
            ),
            (
                "crates/a/Cargo.toml",
                r#"
                    [package]
                    name = "a"
                    license = "MIT"
                "#,
            ),
            ("crates/a/src/lib.rs", ""),
        ]);

        // Low confidence → Guarded (not promoted to Safe)
        let receipts = make_receipt_set_with_evidence(
            "crates/a/src/lib.rs",
            None,
            Some(0.7), // Below 0.9 threshold
            Some(FindingContext {
                workspace: Some(WorkspaceContext {
                    all_crates_agree: true,
                    ..Default::default()
                }),
                analysis_depth: None,
            }),
        );

        let ops = LicenseNormalizeFixer
            .plan(&ctx(), &repo, &receipts)
            .expect("plan");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].safety, SafetyClass::Guarded);
    }
}
