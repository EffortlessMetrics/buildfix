use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::plan::PlannedFix;

mod msrv;
mod path_dep_version;
mod resolver_v2;
mod workspace_inheritance;

pub trait Fixer {
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
