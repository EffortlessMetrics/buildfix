use crate::ops::{FixId, Operation, SafetyClass, TriggerKey};
use crate::receipt::{RunInfo, ToolInfo};
use camino::Utf8PathBuf;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildfixPlan {
    pub schema: String,
    pub tool: ToolInfo,
    pub run: RunInfo,

    /// Unique identifier for this plan.
    pub plan_id: String,

    #[serde(default)]
    pub policy: PlanPolicySnapshot,

    pub inputs: PlanInputs,

    /// Receipts considered when generating this plan.
    #[serde(default)]
    pub receipts: Vec<PlanReceiptRef>,

    pub summary: PlanSummary,

    #[serde(default)]
    pub fixes: Vec<PlannedFix>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl BuildfixPlan {
    pub fn new(tool: ToolInfo, inputs: PlanInputs, policy: PlanPolicySnapshot) -> Self {
        Self {
            schema: crate::schema::BUILDFIX_PLAN_V1.to_string(),
            tool,
            run: RunInfo {
                started_at: Some(Utc::now()),
                ended_at: None,
                git_head_sha: None,
            },
            plan_id: Uuid::new_v4().to_string(),
            policy,
            inputs,
            receipts: vec![],
            summary: PlanSummary::default(),
            fixes: vec![],
            notes: vec![],
        }
    }
}

/// Policy caps that limit the scope of a plan.
///
/// These caps are checked during plan generation and cause a policy block (exit 2)
/// if exceeded.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyCaps {
    /// Maximum number of operations allowed in the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ops: Option<u64>,

    /// Maximum number of files that can be modified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_files: Option<u64>,

    /// Maximum total bytes of patch output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_patch_bytes: Option<u64>,
}

impl PolicyCaps {
    /// Returns true if no caps are configured.
    pub fn is_empty(&self) -> bool {
        self.max_ops.is_none() && self.max_files.is_none() && self.max_patch_bytes.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanPolicySnapshot {
    /// Glob-like patterns of allowed fix ids, applied at *apply* time.
    #[serde(default)]
    pub allow: Vec<String>,

    /// Glob-like patterns of denied fix ids, applied at *apply* time.
    #[serde(default)]
    pub deny: Vec<String>,

    /// Whether apply should refuse if target files have changed since plan.
    #[serde(default)]
    pub require_clean_hashes: bool,

    /// Policy caps limiting the scope of the plan.
    #[serde(default, skip_serializing_if = "PolicyCaps::is_empty")]
    pub caps: PolicyCaps,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanInputs {
    pub repo_root: Utf8PathBuf,
    pub artifacts_dir: Utf8PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanReceiptRef {
    pub sensor_id: String,
    pub report_path: Utf8PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    pub parse_ok: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanSummary {
    pub fixes_total: u64,
    pub safe: u64,
    pub guarded: u64,
    pub unsafe_: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedFix {
    /// Stable identifier for this fix within the plan.
    pub id: String,

    pub fix_id: FixId,
    pub safety: SafetyClass,

    pub title: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default)]
    pub triggers: Vec<FindingRef>,

    pub operations: Vec<Operation>,

    #[serde(default)]
    pub preconditions: Vec<Precondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingRef {
    pub trigger: TriggerKey,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationRef>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationRef {
    pub path: Utf8PathBuf,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Precondition {
    FileExists {
        path: Utf8PathBuf,
    },
    FileSha256 {
        path: Utf8PathBuf,
        sha256: String,
    },
    /// Verifies the git HEAD SHA matches the expected value.
    /// Used to ensure the plan is applied to the same repo state it was generated from.
    GitHeadSha {
        sha: String,
    },
}
