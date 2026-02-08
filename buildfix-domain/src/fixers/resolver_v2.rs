use crate::fixers::{Fixer, FixerMeta};
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{PlanOp, Rationale};
use camino::Utf8PathBuf;
use toml_edit::DocumentMut;

pub struct ResolverV2Fixer;

impl ResolverV2Fixer {
    const FIX_ID: &'static str = "cargo.workspace_resolver_v2";
    const DESCRIPTION: &'static str =
        "Sets [workspace].resolver = \"2\" for correct feature unification";
    const SENSORS: &'static [&'static str] = &["builddiag", "cargo"];
    const CHECK_IDS: &'static [&'static str] =
        &["workspace.resolver_v2", "cargo.workspace.resolver_v2"];

    fn needs_fix(repo: &dyn RepoView, manifest: &Utf8PathBuf) -> bool {
        let contents = match repo.read_to_string(manifest) {
            Ok(c) => c,
            Err(_) => return false,
        };

        let doc = match contents.parse::<DocumentMut>() {
            Ok(d) => d,
            Err(_) => return false,
        };

        let ws = match doc.get("workspace").and_then(|i| i.as_table()) {
            Some(t) => t,
            None => return false, // Not a workspace; resolver-v2 is inapplicable.
        };

        let resolver = ws
            .get("resolver")
            .and_then(|i| i.as_value())
            .and_then(|v| v.as_str());

        resolver != Some("2")
    }
}

impl Fixer for ResolverV2Fixer {
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

        let manifest: Utf8PathBuf = "Cargo.toml".into();
        if !Self::needs_fix(repo, &manifest) {
            return Ok(vec![]);
        }

        let fix_key = triggers
            .first()
            .map(fix_key_for)
            .unwrap_or_else(|| "unknown/-/-".to_string());

        Ok(vec![PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            blocked_reason_token: None,
            target: OpTarget {
                path: manifest.to_string(),
            },
            kind: OpKind::TomlTransform {
                rule_id: "ensure_workspace_resolver_v2".to_string(),
                args: None,
            },
            rationale: Rationale {
                fix_key,
                description: Some(Self::DESCRIPTION.to_string()),
                findings: triggers,
            },
            params_required: vec![],
            preview: None,
        }])
    }
}

fn fix_key_for(f: &buildfix_types::plan::FindingRef) -> String {
    let check = f.check_id.clone().unwrap_or_else(|| "-".to_string());
    format!("{}/{}/{}", f.source, check, f.code)
}
