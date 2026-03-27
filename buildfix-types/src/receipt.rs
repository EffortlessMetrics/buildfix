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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

    /// Confidence score (0.0 to 1.0) indicating certainty of the finding.
    /// Higher values indicate more certainty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,

    /// Provenance chain describing how the finding was derived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,

    /// Context metadata for the finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<FindingContext>,
}

impl Finding {
    /// Returns true if this finding has high confidence (>= 0.9).
    pub fn is_high_confidence(&self) -> bool {
        self.confidence.is_some_and(|c| c >= 0.9)
    }

    /// Returns true if this finding has full consensus across all workspace crates.
    pub fn has_full_consensus(&self) -> bool {
        self.context
            .as_ref()
            .is_some_and(|ctx| ctx.workspace.as_ref().is_some_and(|ws| ws.all_crates_agree))
    }

    /// Returns true if multiple tools agree on this finding.
    pub fn has_tool_agreement(&self) -> bool {
        self.provenance.as_ref().is_some_and(|p| p.agreement)
    }
}

/// Provenance information describing how a finding was derived.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// Method used to derive the finding (e.g., "dead_code_analysis", "license_detection")
    pub method: String,

    /// Tools/sensors that contributed to this finding
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,

    /// Whether multiple tools agree on this finding
    #[serde(default)]
    pub agreement: bool,

    /// Chain of evidence leading to this finding
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_chain: Vec<Evidence>,
}

/// A single piece of evidence in the provenance chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Source of the evidence: "repo", "lockfile", "registry", "workspace", "analysis"
    pub source: String,

    /// The value from this source
    pub value: serde_json::Value,

    /// Whether this evidence was validated
    #[serde(default)]
    pub validated: bool,
}

impl Default for Evidence {
    fn default() -> Self {
        Self {
            source: String::new(),
            value: serde_json::Value::Null,
            validated: false,
        }
    }
}

/// Context metadata for a finding.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FindingContext {
    /// Workspace-wide context for this finding
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceContext>,

    /// Depth of analysis performed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_depth: Option<AnalysisDepth>,
}

/// Workspace-wide context for consensus-based findings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceContext {
    /// Consensus value across all workspace members
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consensus_value: Option<serde_json::Value>,

    /// Number of crates with consensus value
    #[serde(default)]
    pub consensus_count: u64,

    /// Total number of crates analyzed
    #[serde(default)]
    pub total_crates: u64,

    /// Values that differ from consensus
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outliers: Vec<serde_json::Value>,

    /// Crates with outlier values
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outlier_crates: Vec<String>,

    /// Whether all crates agree on the value
    #[serde(default)]
    pub all_crates_agree: bool,
}

/// Depth of analysis performed by the sensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisDepth {
    /// Quick scan, may have false negatives
    Shallow,
    /// Standard analysis
    #[default]
    Full,
    /// Comprehensive analysis with cross-referencing
    Deep,
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
