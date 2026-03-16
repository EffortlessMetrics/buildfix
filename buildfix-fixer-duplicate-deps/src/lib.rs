mod planner {
    #[cfg(test)]
    pub use buildfix_fixer_api::PlannerConfig;
    pub use buildfix_fixer_api::{MatchedFinding, PlanContext, ReceiptSet};
}

mod ports {
    pub use buildfix_fixer_api::RepoView;
}

mod fixers {
    pub use buildfix_fixer_api::{Fixer, FixerMeta};
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/duplicate_deps.rs"
));
