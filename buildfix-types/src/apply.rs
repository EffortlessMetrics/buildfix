use crate::ops::{FixId, SafetyClass};
use crate::plan::Precondition;
use crate::receipt::{RunInfo, ToolInfo};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildfixApply {
    pub schema: String,
    pub tool: ToolInfo,
    pub run: RunInfo,

    pub plan_id: String,

    /// Whether changes were actually applied to disk.
    pub applied: bool,

    #[serde(default)]
    pub summary: ApplySummary,

    #[serde(default)]
    pub results: Vec<AppliedFixResult>,
}

impl BuildfixApply {
    pub fn new(tool: ToolInfo, plan_id: String) -> Self {
        Self {
            schema: crate::schema::BUILDFIX_APPLY_V1.to_string(),
            tool,
            run: RunInfo {
                started_at: Some(Utc::now()),
                ended_at: None,
            },
            plan_id,
            applied: false,
            summary: ApplySummary::default(),
            results: vec![],
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApplySummary {
    pub attempted: u64,
    pub applied: u64,
    pub skipped: u64,
    pub failed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedFixResult {
    pub fix_id: FixId,
    pub fix_instance_id: String,
    pub safety: SafetyClass,

    pub title: String,

    /// Preconditions evaluated before applying.
    #[serde(default)]
    pub preconditions: Vec<PreconditionResult>,

    /// Whether this fix was applied, skipped, or failed.
    pub status: ApplyStatus,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(default)]
    pub files_changed: Vec<FileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyStatus {
    Applied,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreconditionResult {
    pub precondition: Precondition,
    pub ok: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub before_sha256: String,
    pub after_sha256: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_bytes: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_bytes: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_at: Option<DateTime<Utc>>,
}
