use camino::Utf8PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A generic sensor receipt envelope.
///
/// buildfix tries hard to be *tolerant* when reading receipts:
/// - Unknown fields are ignored.
/// - Optional fields may be absent.
///
/// The director and sensors should enforce stricter schema compliance; buildfix's job is to be useful
/// with receipts "as found".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptEnvelope {
    /// Schema identifier, e.g. "buildscan.report.v1".
    pub schema: String,

    pub tool: ToolInfo,

    #[serde(default)]
    pub run: RunInfo,

    #[serde(default)]
    pub verdict: Verdict,

    #[serde(default)]
    pub findings: Vec<Finding>,

    /// Capabilities block for "No Green By Omission" pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ReceiptCapabilities>,

    /// Optional, tool-specific payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Capabilities block describing what the sensor can/did check.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReceiptCapabilities {
    /// List of check_ids this sensor can emit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub check_ids: Vec<String>,

    /// Scopes this sensor covers (e.g., "workspace", "crate").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,

    /// True if some inputs could not be processed.
    #[serde(default)]
    pub partial: bool,

    /// Reason for partial results, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,

    /// Git HEAD SHA at the time this run was created.
    /// Used to verify the plan is applied to the same repo state it was generated from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_head_sha: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Verdict {
    #[serde(default)]
    pub status: VerdictStatus,

    #[serde(default)]
    pub counts: Counts,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictStatus {
    Pass,
    Warn,
    Fail,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Counts {
    #[serde(default)]
    pub findings: u64,

    #[serde(default)]
    pub errors: u64,

    #[serde(default)]
    pub warnings: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    #[serde(default)]
    pub severity: Severity,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,

    /// A stable key (ideally) for deduplication across runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,

    /// Optional, tool-specific payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub path: Utf8PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}
