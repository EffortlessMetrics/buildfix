use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildfixReport {
    pub schema: String,
    pub tool: ReportToolInfo,
    pub run: ReportRunInfo,
    pub verdict: ReportVerdict,

    #[serde(default)]
    pub findings: Vec<ReportFinding>,

    /// Capabilities block for "No Green By Omission" pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ReportCapabilities>,

    /// Pointers to related artifact files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<ReportArtifacts>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportToolInfo {
    pub name: String,
    pub version: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRunInfo {
    pub started_at: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_head_sha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportVerdict {
    pub status: ReportStatus,
    pub counts: ReportCounts,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportCounts {
    pub info: u64,
    pub warn: u64,
    pub error: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportFinding {
    pub severity: ReportSeverity,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,

    pub code: String,
    pub message: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<ReportLocation>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportLocation {
    pub path: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<u64>,
}

/// Capabilities block for "No Green By Omission" pattern.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportCapabilities {
    /// List of check_ids this sensor can emit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub check_ids: Vec<String>,

    /// Scopes this sensor covers, e.g. ['workspace', 'crate'].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,

    /// True if some inputs could not be processed.
    #[serde(default)]
    pub partial: bool,

    /// Reason for partial results, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Successfully loaded input paths/receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs_available: Vec<String>,

    /// Input paths that failed to load.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs_failed: Vec<InputFailure>,
}

/// Record of an input that failed to load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputFailure {
    pub path: String,
    pub reason: String,
}

/// Pointers to related artifact files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportArtifacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}
