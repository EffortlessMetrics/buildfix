use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::SafetyClass;
use buildfix_types::plan::PlannedFix;
use serde::Serialize;

mod msrv;
mod path_dep_version;
mod resolver_v2;
mod workspace_inheritance;

/// Metadata describing a fixer for listing/documentation purposes.
#[derive(Debug, Clone, Serialize)]
pub struct FixerMeta {
    /// Unique key for this fixer (e.g., "cargo.workspace_resolver_v2").
    pub fix_key: &'static str,
    /// Brief human-readable description of what the fixer does.
    pub description: &'static str,
    /// Safety classification for fixes produced by this fixer.
    pub safety: SafetyClass,
    /// Receipt sensors this fixer consumes (tool name prefixes).
    pub consumes_sensors: &'static [&'static str],
    /// Check IDs this fixer looks for in receipts.
    pub consumes_check_ids: &'static [&'static str],
}

pub trait Fixer {
    /// Returns metadata describing this fixer.
    fn meta(&self) -> FixerMeta;

    fn plan(
        &self,
        ctx: &crate::planner::PlanContext,
        repo: &dyn RepoView,
        receipts: &ReceiptSet,
    ) -> anyhow::Result<Vec<PlannedFix>>;
}

pub fn builtin_fixers() -> Vec<Box<dyn Fixer>> {
    vec![
        Box::new(resolver_v2::ResolverV2Fixer),
        Box::new(path_dep_version::PathDepVersionFixer),
        Box::new(workspace_inheritance::WorkspaceInheritanceFixer),
        Box::new(msrv::MsrvNormalizeFixer),
    ]
}

/// Returns metadata for all builtin fixers.
pub fn builtin_fixer_metas() -> Vec<FixerMeta> {
    builtin_fixers().iter().map(|f| f.meta()).collect()
}
