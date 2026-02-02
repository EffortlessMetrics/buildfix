use crate::receipt::{Finding, RunInfo, ToolInfo, Verdict};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildfixReport {
    pub schema: String,
    pub tool: ToolInfo,
    pub run: RunInfo,
    pub verdict: Verdict,

    #[serde(default)]
    pub findings: Vec<Finding>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}
