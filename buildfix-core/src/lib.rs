//! Embeddable core library for buildfix.
//!
//! Provides a clap-free, I/O-abstracted entry point suitable for linking
//! into a cockpit mega-binary or other host process.
//!
//! # Port traits
//!
//! All I/O is abstracted behind port traits in [`ports`]:
//! - [`ReceiptSource`](ports::ReceiptSource) — load sensor receipts
//! - [`GitPort`](ports::GitPort) — query git state
//! - [`WritePort`](ports::WritePort) — write files and create directories
//!
//! The [`adapters`] module provides default filesystem-backed implementations.
//!
//! # Entry points
//!
//! - [`run_plan`](pipeline::run_plan) — generate a plan + report
//! - [`run_apply`](pipeline::run_apply) — apply an existing plan + report

pub mod adapters;
pub mod pipeline;
pub mod ports;
pub mod settings;

// Re-export the domain's RepoView so callers don't need buildfix-domain directly.
pub use buildfix_domain::RepoView;

// Re-export receipt types so embedders don't need buildfix-receipts directly.
pub use buildfix_receipts::{LoadedReceipt, ReceiptEnvelope, ReceiptLoadError};
