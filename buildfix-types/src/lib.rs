//! Shared DTOs (schemas-as-code) for the buildfix workspace.
//!
//! # Design constraints
//! - These types are intended to be serialized to disk.
//! - Be conservative with breaking changes.
//! - Prefer adding optional fields over changing semantics.

pub mod apply;
pub mod ops;
pub mod plan;
pub mod receipt;
pub mod report;

/// Schema identifiers.
pub mod schema {
    pub const BUILDFIX_PLAN_V1: &str = "buildfix.plan.v1";
    pub const BUILDFIX_APPLY_V1: &str = "buildfix.apply.v1";
    pub const BUILDFIX_REPORT_V1: &str = "buildfix.report.v1";
}
