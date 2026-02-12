use crate::receipt::ToolInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildfixApply {
    pub schema: String,
    pub tool: ToolInfo,
    pub repo: ApplyRepoInfo,
    pub plan_ref: PlanRef,
    pub preconditions: ApplyPreconditions,
    #[serde(default)]
    pub results: Vec<ApplyResult>,
    pub summary: ApplySummary,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl BuildfixApply {
    pub fn new(tool: ToolInfo, repo: ApplyRepoInfo, plan_ref: PlanRef) -> Self {
        Self {
            schema: crate::schema::BUILDFIX_APPLY_V1.to_string(),
            tool,
            repo,
            plan_ref,
            preconditions: ApplyPreconditions::default(),
            results: vec![],
            summary: ApplySummary::default(),
            errors: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyRepoInfo {
    pub root: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_sha_before: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_sha_after: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty_before: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty_after: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRef {
    pub path: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApplyPreconditions {
    pub verified: bool,

    #[serde(default)]
    pub mismatches: Vec<PreconditionMismatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreconditionMismatch {
    pub path: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub op_id: String,
    pub status: ApplyStatus,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason_token: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<ApplyFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyStatus {
    Applied,
    Blocked,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyFile {
    pub path: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_before: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_after: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApplySummary {
    pub attempted: u64,
    pub applied: u64,
    pub blocked: u64,
    pub failed: u64,
    pub files_modified: u64,
}
