mod planner {
    #[cfg(test)]
    pub use buildfix_fixer_api::PlannerConfig;
    pub use buildfix_fixer_api::{PlanContext, ReceiptSet};
}

mod ports {
    pub use buildfix_fixer_api::RepoView;
}

mod fixers {
    pub use buildfix_fixer_api::{Fixer, FixerMeta};
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/path_dep_version.rs"
));
