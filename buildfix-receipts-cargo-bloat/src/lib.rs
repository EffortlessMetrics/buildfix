use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

const SIZE_THRESHOLD_BYTES: u64 = 50_000;

pub struct CargoBloatAdapter {
    sensor_id: String,
}

impl CargoBloatAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-bloat".to_string(),
        }
    }
}

impl Default for CargoBloatAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoBloatAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_bloat_json(&content, &self.sensor_id)
    }
}

fn convert_cargo_bloat_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let parsed: serde_json::Value = serde_json::from_str(content).map_err(AdapterError::Json)?;
    let report: CargoBloatReport = serde_json::from_value(parsed).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();

    if let Some(ref crates) = report.crate_info {
        for krate in crates {
            if krate.size >= SIZE_THRESHOLD_BYTES {
                let check_id = if krate.is_lib {
                    "size.large_crate".to_string()
                } else {
                    "bloat.large_dependency".to_string()
                };

                let message = if krate.is_lib {
                    format!(
                        "Crate '{}' contributes {} bytes ({}) to binary size",
                        krate.name, krate.size, krate.percent
                    )
                } else {
                    format!(
                        "Dependency '{}' contributes {} bytes ({}) to binary size",
                        krate.name, krate.size, krate.percent
                    )
                };

                findings.push(Finding {
                    severity: Severity::Info,
                    check_id: Some(check_id),
                    code: None,
                    message: Some(message),
                    location: Some(Location {
                        path: Utf8PathBuf::from("Cargo.toml"),
                        line: None,
                        column: None,
                    }),
                    fingerprint: None,
                    data: Some(serde_json::json!({
                        "crate_name": krate.name,
                        "size_bytes": krate.size,
                        "percent": krate.percent,
                        "is_lib": krate.is_lib,
                    })),
                });
            }
        }
    }

    let status = if !findings.is_empty() {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-bloat.report.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, findings.len() as u64);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    let mut receipt = builder.build();
    receipt.data = Some(serde_json::json!({
        "analyzed": report.analyzed,
        "total_size": report.total_size,
        "crate_count": report.crate_info.as_ref().map(|c| c.len()).unwrap_or(0),
        "top_functions_count": report.top_functions.as_ref().map(|f| f.len()).unwrap_or(0),
    }));

    Ok(receipt)
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CargoBloatReport {
    analyzed: String,
    total_size: u64,
    crate_info: Option<Vec<CrateInfo>>,
    top_functions: Option<Vec<TopFunction>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CrateInfo {
    name: String,
    size: u64,
    percent: String,
    #[serde(rename = "is_lib")]
    is_lib: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TopFunction {
    name: String,
    size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoBloatAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-bloat");
    }

    #[test]
    fn test_convert_cargo_bloat_json_with_large_crates() {
        let json = r#"{
          "analyzed": "target/release/myapp",
          "total_size": 1000000,
          "crate_info": [
            {"name": "serde", "size": 50000, "percent": "5.0", "is_lib": true},
            {"name": "tokio", "size": 80000, "percent": "8.0", "is_lib": true},
            {"name": "small", "size": 1000, "percent": "0.1", "is_lib": true}
          ],
          "top_functions": [
            {"name": "serde::serialize", "size": 10000},
            {"name": "tokio::run", "size": 8000}
          ]
        }"#;

        let receipt = convert_cargo_bloat_json(json, "cargo-bloat").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);

        let serde_finding = &receipt.findings[0];
        assert_eq!(serde_finding.check_id, Some("size.large_crate".to_string()));
        assert!(serde_finding.message.as_ref().unwrap().contains("serde"));
        assert_eq!(serde_finding.severity, Severity::Info);

        let tokio_finding = &receipt.findings[1];
        assert_eq!(tokio_finding.check_id, Some("size.large_crate".to_string()));
        assert!(tokio_finding.message.as_ref().unwrap().contains("tokio"));
    }

    #[test]
    fn test_convert_cargo_bloat_json_no_large_crates() {
        let json = r#"{
          "analyzed": "target/release/myapp",
          "total_size": 100000,
          "crate_info": [
            {"name": "small", "size": 1000, "percent": "1.0", "is_lib": true}
          ],
          "top_functions": []
        }"#;

        let receipt = convert_cargo_bloat_json(json, "cargo-bloat").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_bloat_json_non_lib_dependency() {
        let json = r#"{
          "analyzed": "target/release/myapp",
          "total_size": 1000000,
          "crate_info": [
            {"name": "some_binary_dep", "size": 60000, "percent": "6.0", "is_lib": false}
          ],
          "top_functions": []
        }"#;

        let receipt = convert_cargo_bloat_json(json, "cargo-bloat").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.check_id, Some("bloat.large_dependency".to_string()));
        assert!(finding.message.as_ref().unwrap().contains("Dependency"));
    }

    #[test]
    fn test_convert_cargo_bloat_json_empty() {
        let json = r#"{
          "analyzed": "target/release/myapp",
          "total_size": 1000,
          "crate_info": [],
          "top_functions": []
        }"#;

        let receipt = convert_cargo_bloat_json(json, "cargo-bloat").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_bloat_json_null_fields() {
        let json = r#"{
          "analyzed": "target/release/myapp",
          "total_size": 1000,
          "crate_info": null,
          "top_functions": null
        }"#;

        let receipt = convert_cargo_bloat_json(json, "cargo-bloat").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }
}
