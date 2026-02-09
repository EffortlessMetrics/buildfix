//! Error types for buildfix-edit.
//!
//! This module defines error types that distinguish between:
//! - Policy blocks (exit code 2): precondition mismatch, safety gate denial, allowlist/denylist
//! - Runtime errors (exit code 1): I/O errors, parse errors, invalid arguments

use thiserror::Error;

/// The top-level error type for buildfix-edit operations.
#[derive(Debug, Error)]
pub enum EditError {
    /// A policy block occurred (exit code 2).
    /// This includes precondition failures, safety gate denials, and policy denials.
    #[error("policy block: {0}")]
    PolicyBlock(#[from] PolicyBlockError),

    /// A runtime/tool error occurred (exit code 1).
    /// This includes I/O errors, parse errors, and invalid arguments.
    #[error("runtime error: {0}")]
    Runtime(#[from] anyhow::Error),
}

/// Policy block errors that should result in exit code 2.
#[derive(Debug, Error)]
pub enum PolicyBlockError {
    /// One or more preconditions failed (file changed, missing, sha256 mismatch).
    #[error("precondition mismatch: {message}")]
    PreconditionMismatch {
        /// A descriptive message about which preconditions failed.
        message: String,
    },

    /// A fix was denied by the safety gate (guarded/unsafe not allowed).
    #[error("safety gate denial: {message}")]
    SafetyGateDenial {
        /// A descriptive message about which safety class was blocked.
        message: String,
    },

    /// A fix was denied by the allow/deny policy.
    #[error("policy denial: {message}")]
    PolicyDenial {
        /// A descriptive message about which policy blocked the fix.
        message: String,
    },

    /// Caps exceeded (max operations, max files, max diff size).
    #[error("caps exceeded: {message}")]
    CapsExceeded {
        /// A descriptive message about which cap was exceeded.
        message: String,
    },
}

impl EditError {
    /// Returns true if this is a policy block error (exit code 2).
    pub fn is_policy_block(&self) -> bool {
        matches!(self, EditError::PolicyBlock(_))
    }

    /// Returns the recommended exit code for this error.
    pub fn exit_code(&self) -> u8 {
        match self {
            EditError::PolicyBlock(_) => 2,
            EditError::Runtime(_) => 1,
        }
    }
}

/// Result type alias using EditError.
pub type EditResult<T> = Result<T, EditError>;

#[cfg(test)]
mod tests {
    use super::{EditError, PolicyBlockError};

    #[test]
    fn policy_block_reports_exit_code_2() {
        let err = EditError::from(PolicyBlockError::PreconditionMismatch {
            message: "oops".to_string(),
        });
        assert!(err.is_policy_block());
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("policy block"));
    }

    #[test]
    fn runtime_error_reports_exit_code_1() {
        let err = EditError::from(anyhow::anyhow!("boom"));
        assert!(!err.is_policy_block());
        assert_eq!(err.exit_code(), 1);
        assert!(err.to_string().contains("runtime error"));
    }

    #[test]
    fn policy_block_display_includes_variant() {
        let err = PolicyBlockError::SafetyGateDenial {
            message: "guarded".to_string(),
        };
        assert!(err.to_string().contains("safety gate denial"));
        assert!(err.to_string().contains("guarded"));
    }
}
