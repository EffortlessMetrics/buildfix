use serde::{Deserialize, Serialize};

use crate::report::{
    BuildfixReport, ReportArtifacts, ReportCapabilities, ReportFinding, ReportRunInfo,
    ReportVerdict,
};
use crate::wire::ToolInfoV1;

/// Schema-exact wire representation of buildfix.report.v1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportV1 {
    pub schema: String,
    pub tool: ToolInfoV1,
    pub run: ReportRunInfo,
    pub verdict: ReportVerdict,

    #[serde(default)]
    pub findings: Vec<ReportFinding>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ReportCapabilities>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<ReportArtifacts>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl From<&BuildfixReport> for ReportV1 {
    fn from(report: &BuildfixReport) -> Self {
        Self {
            schema: report.schema.clone(),
            tool: ToolInfoV1 {
                name: report.tool.name.clone(),
                version: report.tool.version.clone(),
                commit: report.tool.commit.clone(),
            },
            run: report.run.clone(),
            verdict: report.verdict.clone(),
            findings: report.findings.clone(),
            capabilities: report.capabilities.clone(),
            artifacts: report.artifacts.clone(),
            data: report.data.clone(),
        }
    }
}
