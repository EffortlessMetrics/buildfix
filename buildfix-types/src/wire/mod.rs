use serde::{Deserialize, Serialize};

pub mod apply_v1;
pub mod plan_v1;
pub mod report_v1;

pub use apply_v1::ApplyV1;
pub use plan_v1::PlanV1;
pub use report_v1::ReportV1;

/// Tool information for wire-level schemas (schema-exact).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfoV1 {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// Errors emitted while converting internal models to wire models.
#[derive(Debug, Clone)]
pub enum WireError {
    MissingToolVersion { context: &'static str },
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::MissingToolVersion { context } => {
                write!(f, "missing tool version for {}", context)
            }
        }
    }
}

impl std::error::Error for WireError {}
