use serde::{Deserialize, Serialize};

use crate::apply::{
    ApplyPreconditions, ApplyRepoInfo, ApplyResult, ApplySummary, BuildfixApply, PlanRef,
};
use crate::receipt::ToolInfo;
use crate::wire::{ToolInfoV1, WireError};

/// Schema-exact wire representation of buildfix.apply.v1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyV1 {
    pub schema: String,
    pub tool: ToolInfoV1,
    pub repo: ApplyRepoInfo,
    pub plan_ref: PlanRef,
    pub preconditions: ApplyPreconditions,

    #[serde(default)]
    pub results: Vec<ApplyResult>,

    pub summary: ApplySummary,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl TryFrom<&BuildfixApply> for ApplyV1 {
    type Error = WireError;

    fn try_from(apply: &BuildfixApply) -> Result<Self, Self::Error> {
        let version = apply
            .tool
            .version
            .clone()
            .ok_or(WireError::MissingToolVersion { context: "apply" })?;

        Ok(Self {
            schema: apply.schema.clone(),
            tool: ToolInfoV1 {
                name: apply.tool.name.clone(),
                version,
                commit: apply.tool.commit.clone(),
            },
            repo: apply.repo.clone(),
            plan_ref: apply.plan_ref.clone(),
            preconditions: apply.preconditions.clone(),
            results: apply.results.clone(),
            summary: apply.summary.clone(),
            errors: apply.errors.clone(),
        })
    }
}

impl From<ApplyV1> for BuildfixApply {
    fn from(apply: ApplyV1) -> Self {
        BuildfixApply {
            schema: apply.schema,
            tool: ToolInfo {
                name: apply.tool.name,
                version: Some(apply.tool.version),
                repo: None,
                commit: apply.tool.commit,
            },
            repo: apply.repo,
            plan_ref: apply.plan_ref,
            preconditions: apply.preconditions,
            results: apply.results,
            summary: apply.summary,
            errors: apply.errors,
        }
    }
}
