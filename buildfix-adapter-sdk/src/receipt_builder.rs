//! Helper utilities for building receipt envelopes in tests and adapters.
//!
//! This module provides a builder pattern for constructing `ReceiptEnvelope`
//! instances with sensible defaults. It's useful when creating test fixtures
//! or implementing adapter logic that needs to construct receipts.

use buildfix_types::receipt::{
    Counts, Finding, Location, ReceiptCapabilities, ReceiptEnvelope, RunInfo, Severity, ToolInfo,
    Verdict, VerdictStatus,
};
use camino::Utf8PathBuf;

pub struct ReceiptBuilder {
    schema: String,
    tool_name: String,
    tool_version: Option<String>,
    tool_repo: Option<String>,
    tool_commit: Option<String>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    ended_at: Option<chrono::DateTime<chrono::Utc>>,
    git_head_sha: Option<String>,
    status: VerdictStatus,
    findings_count: u64,
    errors_count: u64,
    warnings_count: u64,
    reasons: Vec<String>,
    check_ids: Vec<String>,
    scopes: Vec<String>,
    partial: bool,
    partial_reason: Option<String>,
    findings: Vec<Finding>,
}

impl Default for ReceiptBuilder {
    fn default() -> Self {
        Self {
            schema: "sensor.report.v1".to_string(),
            tool_name: "unknown".to_string(),
            tool_version: None,
            tool_repo: None,
            tool_commit: None,
            started_at: None,
            ended_at: None,
            git_head_sha: None,
            status: VerdictStatus::Unknown,
            findings_count: 0,
            errors_count: 0,
            warnings_count: 0,
            reasons: Vec::new(),
            check_ids: Vec::new(),
            scopes: Vec::new(),
            partial: false,
            partial_reason: None,
            findings: Vec::new(),
        }
    }
}

impl ReceiptBuilder {
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self::default().with_tool_name(tool_name)
    }

    pub fn with_tool_name(mut self, name: impl Into<String>) -> Self {
        self.tool_name = name.into();
        self
    }

    pub fn with_tool_version(mut self, version: impl Into<String>) -> Self {
        self.tool_version = Some(version.into());
        self
    }

    pub fn with_tool_repo(mut self, repo: impl Into<String>) -> Self {
        self.tool_repo = Some(repo.into());
        self
    }

    pub fn with_tool_commit(mut self, commit: impl Into<String>) -> Self {
        self.tool_commit = Some(commit.into());
        self
    }

    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = schema.into();
        self
    }

    pub fn with_started_at(mut self, time: chrono::DateTime<chrono::Utc>) -> Self {
        self.started_at = Some(time);
        self
    }

    pub fn with_ended_at(mut self, time: chrono::DateTime<chrono::Utc>) -> Self {
        self.ended_at = Some(time);
        self
    }

    pub fn with_git_head_sha(mut self, sha: impl Into<String>) -> Self {
        self.git_head_sha = Some(sha.into());
        self
    }

    pub fn with_status(mut self, status: VerdictStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_counts(mut self, findings: u64, errors: u64, warnings: u64) -> Self {
        self.findings_count = findings;
        self.errors_count = errors;
        self.warnings_count = warnings;
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reasons.push(reason.into());
        self
    }

    pub fn with_check_id(mut self, check_id: impl Into<String>) -> Self {
        self.check_ids.push(check_id.into());
        self
    }

    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scopes.push(scope.into());
        self
    }

    pub fn with_partial(mut self, partial: bool) -> Self {
        self.partial = partial;
        self
    }

    pub fn with_partial_reason(mut self, reason: impl Into<String>) -> Self {
        self.partial_reason = Some(reason.into());
        self
    }

    pub fn with_finding(mut self, finding: Finding) -> Self {
        self.findings.push(finding);
        self
    }

    pub fn with_finding_at(
        mut self,
        path: impl Into<Utf8PathBuf>,
        line: u64,
        message: impl Into<String>,
        severity: Severity,
    ) -> Self {
        let finding = Finding {
            severity,
            check_id: None,
            code: None,
            message: Some(message.into()),
            location: Some(Location {
                path: path.into(),
                line: Some(line),
                column: None,
            }),
            fingerprint: None,
            data: None,
            confidence: None,
            provenance: None,
            context: None,
        };
        self.findings.push(finding);
        self
    }

    pub fn build(self) -> ReceiptEnvelope {
        let findings_count = if self.findings_count == 0 {
            self.findings.len() as u64
        } else {
            self.findings_count
        };

        let capabilities = if self.check_ids.is_empty() && self.scopes.is_empty() && !self.partial {
            None
        } else {
            Some(ReceiptCapabilities {
                check_ids: self.check_ids,
                scopes: self.scopes,
                partial: self.partial,
                reason: self.partial_reason,
            })
        };

        ReceiptEnvelope {
            schema: self.schema,
            tool: ToolInfo {
                name: self.tool_name,
                version: self.tool_version,
                repo: self.tool_repo,
                commit: self.tool_commit,
            },
            run: RunInfo {
                started_at: self.started_at,
                ended_at: self.ended_at,
                git_head_sha: self.git_head_sha,
            },
            verdict: Verdict {
                status: self.status,
                counts: Counts {
                    findings: findings_count,
                    errors: self.errors_count,
                    warnings: self.warnings_count,
                },
                reasons: self.reasons,
            },
            findings: self.findings,
            capabilities,
            data: None,
        }
    }
}

pub fn simple_finding(
    message: impl Into<String>,
    path: impl Into<Utf8PathBuf>,
    line: u64,
    severity: Severity,
) -> Finding {
    Finding {
        severity,
        check_id: None,
        code: None,
        message: Some(message.into()),
        location: Some(Location {
            path: path.into(),
            line: Some(line),
            column: None,
        }),
        fingerprint: None,
        data: None,
        confidence: None,
        provenance: None,
        context: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buildfix_types::receipt::Severity;

    #[test]
    fn test_receipt_builder_basic() {
        let receipt = ReceiptBuilder::new("test-sensor")
            .with_tool_version("1.0.0")
            .with_status(VerdictStatus::Fail)
            .with_counts(2, 1, 1)
            .with_finding_at("Cargo.toml", 5, "Test error", Severity::Error)
            .build();

        assert_eq!(receipt.tool.name, "test-sensor");
        assert_eq!(receipt.tool.version, Some("1.0.0".to_string()));
        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.findings.len(), 1);
    }

    #[test]
    fn test_receipt_builder_finding_count() {
        let receipt = ReceiptBuilder::new("test-sensor")
            .with_finding(simple_finding("err1", "foo.rs", 10, Severity::Error))
            .with_finding(simple_finding("err2", "bar.rs", 20, Severity::Error))
            .build();

        assert_eq!(receipt.verdict.counts.findings, 2);
    }

    #[test]
    fn test_receipt_builder_capabilities() {
        let receipt = ReceiptBuilder::new("test-sensor")
            .with_check_id("DENY001")
            .with_check_id("DENY002")
            .with_scope("workspace")
            .with_partial(true)
            .with_partial_reason("Some files could not be parsed")
            .build();

        let caps = receipt.capabilities.expect("capabilities should be set");
        assert_eq!(caps.check_ids.len(), 2);
        assert_eq!(caps.scopes, vec!["workspace"]);
        assert!(caps.partial);
        assert_eq!(
            caps.reason,
            Some("Some files could not be parsed".to_string())
        );
    }
}
