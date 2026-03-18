use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

const LINES_THRESHOLD: u64 = 10000;

pub struct CargoLlvmLinesAdapter {
    sensor_id: String,
}

impl CargoLlvmLinesAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-llvm-lines".to_string(),
        }
    }
}

impl Default for CargoLlvmLinesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoLlvmLinesAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_llvm_lines_json(&content, &self.sensor_id)
    }
}

fn convert_cargo_llvm_lines_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let parsed: serde_json::Value = serde_json::from_str(content).map_err(AdapterError::Json)?;
    let report: LlvmLinesReport = serde_json::from_value(parsed).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();

    if let Some(ref data) = report.data {
        for item in data {
            if item.lines >= LINES_THRESHOLD {
                let message = format!(
                    "Function '{}' generates {} LLVM IR lines ({}%) across {} instance(s)",
                    item.name, item.lines, item.percent, item.instances
                );

                let location = item.paths.as_ref().and_then(|paths| {
                    paths.first().map(|p| Location {
                        path: Utf8PathBuf::from(p.as_str()),
                        line: None,
                        column: None,
                    })
                });

                findings.push(Finding {
                    severity: Severity::Info,
                    check_id: Some("llvm_lines.slow".to_string()),
                    code: None,
                    message: Some(message),
                    location,
                    fingerprint: None,
                    data: Some(serde_json::json!({
                        "name": item.name,
                        "paths": item.paths,
                        "sessions": item.sessions,
                        "lines": item.lines,
                        "instances": item.instances,
                        "percent": item.percent,
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
        .with_schema("cargo-llvm-lines.report.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, findings.len() as u64);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    let mut receipt = builder.build();
    receipt.data = Some(serde_json::json!({
        "total_lines": report.total_lines,
        "analyzed_items": report.data.as_ref().map(|d| d.len()).unwrap_or(0),
    }));

    Ok(receipt)
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LlvmLinesReport {
    data: Option<Vec<LlvmLinesEntry>>,
    #[serde(rename = "total_lines")]
    total_lines: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LlvmLinesEntry {
    name: String,
    paths: Option<Vec<String>>,
    sessions: u64,
    lines: u64,
    instances: u64,
    percent: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoLlvmLinesAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-llvm-lines");
    }

    #[test]
    fn test_convert_llvm_lines_json_with_slow_functions() {
        let json = r#"{
          "data": [
            {
              "name": "serde::ser::serialize",
              "paths": ["src/serialize.rs"],
              "sessions": 1,
              "lines": 50000,
              "instances": 10,
              "percent": "15.0"
            },
            {
              "name": "tokio::runtime::block_on",
              "paths": ["src/runtime.rs"],
              "sessions": 1,
              "lines": 30000,
              "instances": 5,
              "percent": "9.0"
            },
            {
              "name": "small_function",
              "paths": ["src/lib.rs"],
              "sessions": 1,
              "lines": 1000,
              "instances": 1,
              "percent": "0.3"
            }
          ],
          "total_lines": 333333
        }"#;

        let receipt = convert_cargo_llvm_lines_json(json, "cargo-llvm-lines").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);

        let serde_finding = &receipt.findings[0];
        assert_eq!(serde_finding.check_id, Some("llvm_lines.slow".to_string()));
        assert!(
            serde_finding
                .message
                .as_ref()
                .unwrap()
                .contains("serde::ser::serialize")
        );
        assert_eq!(serde_finding.severity, Severity::Info);
        assert_eq!(
            serde_finding.location.as_ref().unwrap().path.as_str(),
            "src/serialize.rs"
        );

        let tokio_finding = &receipt.findings[1];
        assert_eq!(tokio_finding.check_id, Some("llvm_lines.slow".to_string()));
        assert!(
            tokio_finding
                .message
                .as_ref()
                .unwrap()
                .contains("tokio::runtime::block_on")
        );
    }

    #[test]
    fn test_convert_llvm_lines_json_no_slow_functions() {
        let json = r#"{
          "data": [
            {
              "name": "small_function",
              "paths": ["src/lib.rs"],
              "sessions": 1,
              "lines": 1000,
              "instances": 1,
              "percent": "0.3"
            }
          ],
          "total_lines": 333333
        }"#;

        let receipt = convert_cargo_llvm_lines_json(json, "cargo-llvm-lines").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_llvm_lines_json_empty_data() {
        let json = r#"{
          "data": [],
          "total_lines": 0
        }"#;

        let receipt = convert_cargo_llvm_lines_json(json, "cargo-llvm-lines").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_llvm_lines_json_null_data() {
        let json = r#"{
          "data": null,
          "total_lines": 0
        }"#;

        let receipt = convert_cargo_llvm_lines_json(json, "cargo-llvm-lines").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_llvm_lines_json_no_paths() {
        let json = r#"{
          "data": [
            {
              "name": "function_no_path",
              "sessions": 1,
              "lines": 50000,
              "instances": 10,
              "percent": "15.0"
            }
          ],
          "total_lines": 333333
        }"#;

        let receipt = convert_cargo_llvm_lines_json(json, "cargo-llvm-lines").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert!(finding.location.is_none());
    }

    #[test]
    fn test_convert_llvm_lines_json_boundary_threshold() {
        let json = r#"{
          "data": [
            {
              "name": "exactly_threshold",
              "paths": ["src/lib.rs"],
              "sessions": 1,
              "lines": 10000,
              "instances": 1,
              "percent": "3.0"
            },
            {
              "name": "just_above_threshold",
              "paths": ["src/lib.rs"],
              "sessions": 1,
              "lines": 10001,
              "instances": 1,
              "percent": "3.0"
            },
            {
              "name": "just_below_threshold",
              "paths": ["src/lib.rs"],
              "sessions": 1,
              "lines": 9999,
              "instances": 1,
              "percent": "3.0"
            }
          ],
          "total_lines": 333333
        }"#;

        let receipt = convert_cargo_llvm_lines_json(json, "cargo-llvm-lines").unwrap();

        assert_eq!(receipt.findings.len(), 2);
    }
}
