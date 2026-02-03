//! Domain logic: turn receipts + repo state into a deterministic fix plan.
//!
//! This crate owns *what* should be fixed and why. It does not own *how* edits are applied; that's
//! the `buildfix-edit` crate.

mod fixers;
mod planner;
mod ports;

pub use fixers::{builtin_fixer_metas, FixerMeta};
pub use planner::{PlanContext, Planner, PlannerConfig};
pub use ports::{FsRepoView, RepoView};
