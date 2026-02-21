//! Runtime interfaces and launch settings for buildfix.
//!
//! This crate intentionally has a small responsibility:
//! wiring ports/adapters and execution policy configuration that are reused across host binaries.

pub mod adapters;
pub mod ports;
pub mod settings;

#[cfg(feature = "memory")]
pub use adapters::InMemoryReceiptSource;
#[cfg(feature = "git")]
pub use adapters::ShellGitPort;
#[cfg(feature = "fs")]
pub use adapters::{FsReceiptSource, FsWritePort};
pub use ports::{GitPort, ReceiptSource, WritePort};
pub use settings::{ApplySettings, PlanSettings, RunMode};
