use crate::fixers::Fixer;
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{FixId, Operation, SafetyClass};
use buildfix_types::plan::PlannedFix;
use camino::Utf8PathBuf;
use toml_edit::DocumentMut;

pub struct ResolverV2Fixer;

impl ResolverV2Fixer {
    const FIX_ID: &'static str = "cargo.workspace_resolver_v2";

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
            None => return true,
        };

        let resolver = ws
            .get("resolver")
            .and_then(|i| i.as_value())
            .and_then(|v| v.as_str());

        resolver != Some("2")
    }
}

impl Fixer for ResolverV2Fixer {
    fn plan(
        &self,
        _ctx: &crate::planner::PlanContext,
        repo: &dyn RepoView,
        receipts: &ReceiptSet,
    ) -> anyhow::Result<Vec<PlannedFix>> {
        let triggers = receipts.matching_findings(
            &["builddiag", "cargo"],
            &["workspace.resolver_v2", "cargo.workspace.resolver_v2"],
            &[],
        );
        if triggers.is_empty() {
            return Ok(vec![]);
        }

        let manifest: Utf8PathBuf = "Cargo.toml".into();
        if !Self::needs_fix(repo, &manifest) {
            return Ok(vec![]);
        }

        Ok(vec![PlannedFix {
            id: String::new(),
            fix_id: FixId::new(Self::FIX_ID),
            safety: SafetyClass::Safe,
            title: "Set [workspace].resolver = "2"".to_string(),
            description: Some(
                "Cargo's resolver v2 is required for correct feature unification in many modern workspaces."
                    .to_string(),
            ),
            triggers,
            operations: vec![Operation::EnsureWorkspaceResolverV2 { manifest }],
            preconditions: vec![],
        }])
    }
}
