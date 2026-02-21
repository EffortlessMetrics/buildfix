use anyhow::Result;
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::SafetyClass;
use buildfix_types::plan::FindingRef;
use serde::Serialize;

/// Metadata describing a fixer for listing/documentation.
#[derive(Debug, Clone, Serialize)]
pub struct FixerMeta {
    /// Unique key for this fixer (e.g., "cargo.workspace_resolver_v2").
    pub fix_key: &'static str,
    /// Brief human-readable description.
    pub description: &'static str,
    /// Safety classification for this fixer's ops.
    pub safety: SafetyClass,
    /// Tool prefixes consumed by this fixer's checks.
    pub consumes_sensors: &'static [&'static str],
    /// Check IDs consumed by this fixer's checks.
    pub consumes_check_ids: &'static [&'static str],
}

/// Shared repository view used by all fixers.
pub trait RepoView {
    fn root(&self) -> &camino::Utf8Path;

    fn read_to_string(&self, rel: &camino::Utf8Path) -> Result<String>;

    fn exists(&self, rel: &camino::Utf8Path) -> bool;
}

/// Shared planning input passed into fixers.
#[derive(Debug, Clone, Default)]
pub struct PlannerConfig {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub allow_guarded: bool,
    pub allow_unsafe: bool,
    pub allow_dirty: bool,
    pub max_ops: Option<u64>,
    pub max_files: Option<u64>,
    pub max_patch_bytes: Option<u64>,
    pub params: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PlanContext {
    pub repo_root: camino::Utf8PathBuf,
    pub artifacts_dir: camino::Utf8PathBuf,
    pub config: PlannerConfig,
}

/// Contract each fixer implements.
pub trait Fixer {
    fn meta(&self) -> FixerMeta;

    fn plan(
        &self,
        ctx: &PlanContext,
        repo: &dyn RepoView,
        receipts: &ReceiptSet,
    ) -> anyhow::Result<Vec<buildfix_types::plan::PlanOp>>;
}

#[derive(Debug, Clone)]
pub struct MatchedFinding {
    pub finding: FindingRef,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ReceiptRecord {
    #[allow(dead_code)]
    pub sensor_id: String,
    pub path: camino::Utf8PathBuf,
    pub envelope: buildfix_types::receipt::ReceiptEnvelope,
}

/// In-memory queryable set of loaded receipts.
#[derive(Debug, Clone)]
pub struct ReceiptSet {
    receipts: Vec<ReceiptRecord>,
}

impl ReceiptSet {
    pub fn from_loaded(loaded: &[LoadedReceipt]) -> Self {
        let mut receipts = Vec::new();
        for r in loaded {
            if let Ok(env) = &r.receipt {
                receipts.push(ReceiptRecord {
                    sensor_id: r.sensor_id.clone(),
                    path: r.path.clone(),
                    envelope: env.clone(),
                });
            }
        }
        receipts.sort_by(|a, b| a.path.cmp(&b.path));
        Self { receipts }
    }

    pub fn matching_findings(
        &self,
        tool_prefixes: &[&str],
        check_ids: &[&str],
        codes: &[&str],
    ) -> Vec<FindingRef> {
        self.matching_findings_with_data(tool_prefixes, check_ids, codes)
            .into_iter()
            .map(|m| m.finding)
            .collect()
    }

    pub fn matching_findings_with_data(
        &self,
        tool_prefixes: &[&str],
        check_ids: &[&str],
        codes: &[&str],
    ) -> Vec<MatchedFinding> {
        let mut out = Vec::new();

        for r in &self.receipts {
            let tool = r.envelope.tool.name.as_str();
            if !tool_prefixes.iter().any(|p| tool.starts_with(p)) {
                continue;
            }

            for f in &r.envelope.findings {
                let check_ok = if check_ids.is_empty() {
                    true
                } else {
                    f.check_id
                        .as_deref()
                        .map(|c| check_ids.contains(&c))
                        .unwrap_or(false)
                };

                let code_ok = if codes.is_empty() {
                    true
                } else {
                    f.code
                        .as_deref()
                        .map(|c| codes.contains(&c))
                        .unwrap_or(false)
                };

                if !check_ok || !code_ok {
                    continue;
                }

                out.push(MatchedFinding {
                    finding: FindingRef {
                        source: tool.to_string(),
                        check_id: f.check_id.clone(),
                        code: f.code.clone().unwrap_or_else(|| "-".to_string()),
                        path: f.location.as_ref().map(|loc| loc.path.to_string()),
                        line: f.location.as_ref().and_then(|loc| loc.line),
                        fingerprint: f.fingerprint.clone(),
                    },
                    data: f.data.clone(),
                });
            }
        }

        out.sort_by_key(|m| stable_finding_key(&m.finding));
        out
    }
}

fn stable_finding_key(f: &FindingRef) -> String {
    let loc = f
        .path
        .as_ref()
        .map(|p| format!("{}:{}", p, f.line.unwrap_or(0)))
        .unwrap_or_else(|| "no_location".to_string());

    format!(
        "{}/{}/{}|{}",
        f.source,
        f.check_id.clone().unwrap_or_default(),
        f.code,
        loc
    )
}
