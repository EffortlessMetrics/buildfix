use crate::ops::{OpKind, OpPreview, OpTarget, SafetyClass};
use crate::receipt::ToolInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildfixPlan {
    pub schema: String,
    pub tool: ToolInfo,
    pub repo: RepoInfo,

    #[serde(default)]
    pub inputs: Vec<PlanInput>,

    pub policy: PlanPolicy,

    #[serde(default)]
    pub preconditions: PlanPreconditions,

    #[serde(default)]
    pub ops: Vec<PlanOp>,

    pub summary: PlanSummary,
}

impl BuildfixPlan {
    pub fn new(tool: ToolInfo, repo: RepoInfo, policy: PlanPolicy) -> Self {
        Self {
            schema: crate::schema::BUILDFIX_PLAN_V1.to_string(),
            tool,
            repo,
            inputs: vec![],
            policy,
            preconditions: PlanPreconditions::default(),
            ops: vec![],
            summary: PlanSummary::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub root: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_sha: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanInput {
    pub path: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanPolicy {
    #[serde(default)]
    pub allow: Vec<String>,

    #[serde(default)]
    pub deny: Vec<String>,

    #[serde(default)]
    pub allow_guarded: bool,

    #[serde(default)]
    pub allow_unsafe: bool,

    #[serde(default)]
    pub allow_dirty: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ops: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_files: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_patch_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanPreconditions {
    #[serde(default)]
    pub files: Vec<FilePrecondition>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_sha: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePrecondition {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanSummary {
    pub ops_total: u64,
    pub ops_blocked: u64,
    pub files_touched: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOp {
    pub id: String,
    pub safety: SafetyClass,
    pub blocked: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,

    pub target: OpTarget,
    pub kind: OpKind,
    pub rationale: Rationale,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params_required: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<OpPreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rationale {
    pub fix_key: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default)]
    pub findings: Vec<FindingRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingRef {
    pub source: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,

    pub code: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}
