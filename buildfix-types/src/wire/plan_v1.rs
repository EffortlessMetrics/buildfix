use serde::{Deserialize, Serialize};

use crate::plan::{
    BuildfixPlan, PlanInput, PlanOp, PlanPolicy, PlanPreconditions, PlanSummary, RepoInfo,
};
use crate::receipt::ToolInfo;
use crate::wire::{ToolInfoV1, WireError};

/// Schema-exact wire representation of buildfix.plan.v1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanV1 {
    pub schema: String,
    pub tool: ToolInfoV1,
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

impl TryFrom<&BuildfixPlan> for PlanV1 {
    type Error = WireError;

    fn try_from(plan: &BuildfixPlan) -> Result<Self, Self::Error> {
        let version = plan
            .tool
            .version
            .clone()
            .ok_or(WireError::MissingToolVersion { context: "plan" })?;

        Ok(Self {
            schema: plan.schema.clone(),
            tool: ToolInfoV1 {
                name: plan.tool.name.clone(),
                version,
                commit: plan.tool.commit.clone(),
            },
            repo: plan.repo.clone(),
            inputs: plan.inputs.clone(),
            policy: plan.policy.clone(),
            preconditions: plan.preconditions.clone(),
            ops: plan.ops.clone(),
            summary: plan.summary.clone(),
        })
    }
}

impl From<PlanV1> for BuildfixPlan {
    fn from(plan: PlanV1) -> Self {
        BuildfixPlan {
            schema: plan.schema,
            tool: ToolInfo {
                name: plan.tool.name,
                version: Some(plan.tool.version),
                repo: None,
                commit: plan.tool.commit,
            },
            repo: plan.repo,
            inputs: plan.inputs,
            policy: plan.policy,
            preconditions: plan.preconditions,
            ops: plan.ops,
            summary: plan.summary,
        }
    }
}
