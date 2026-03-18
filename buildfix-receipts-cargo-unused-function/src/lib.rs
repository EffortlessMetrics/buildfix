use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoUnusedFunctionAdapter {
    sensor_id: String,
}

impl CargoUnusedFunctionAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-unused-function".to_string(),
        }
    }
}

impl Default for CargoUnusedFunctionAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoUnusedFunctionAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_unused_function_json(&content, &self.sensor_id)
    }
}

fn convert_unused_function_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let input: UnusedFunctionOutput =
        serde_json::from_str(content).map_err(|e| AdapterError::InvalidFormat(e.to_string()))?;

    let mut findings = Vec::new();
    let warning_count = input.functions.len() as u64;

    for func in &input.functions {
        let location = Location {
            path: Utf8PathBuf::from(&func.file),
            line: Some(func.line),
            column: Some(func.column),
        };

        let message = format!(
            "unused function `{}` (visibility: {}) at {}:{}:{}",
            func.name, func.visibility, func.file, func.line, func.column
        );

        findings.push(Finding {
            severity: Severity::Warn,
            check_id: Some("dead_code.unused_function".to_string()),
            code: None,
            message: Some(message),
            location: Some(location),
            fingerprint: None,
            data: None,
        });
    }

    let status = if warning_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-unused-function.report.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, warning_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct UnusedFunctionOutput {
    functions: Vec<UnusedFunction>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct UnusedFunction {
    name: String,
    file: String,
    line: u64,
    column: u64,
    visibility: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoUnusedFunctionAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-unused-function");
    }

    #[test]
    fn test_convert_unused_function_json() {
        let json = r#"{
  "functions": [
    {
      "name": "unused_helper",
      "file": "src/lib.rs",
      "line": 42,
      "column": 4,
      "visibility": "pub"
    }
  ]
}"#;

        let receipt = convert_unused_function_json(json, "cargo-unused-function").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(
            finding.check_id,
            Some("dead_code.unused_function".to_string())
        );
        assert_eq!(
            finding.message.as_ref().unwrap(),
            "unused function `unused_helper` (visibility: pub) at src/lib.rs:42:4"
        );
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "src/lib.rs"
        );
        assert_eq!(finding.location.as_ref().unwrap().line, Some(42));
        assert_eq!(finding.location.as_ref().unwrap().column, Some(4));
    }

    #[test]
    fn test_convert_unused_function_json_multiple_functions() {
        let json = r#"{
  "functions": [
    {
      "name": "unused_helper",
      "file": "src/lib.rs",
      "line": 42,
      "column": 4,
      "visibility": "pub"
    },
    {
      "name": "internal_func",
      "file": "src/utils.rs",
      "line": 10,
      "column": 1,
      "visibility": "pub(crate)"
    }
  ]
}"#;

        let receipt = convert_unused_function_json(json, "cargo-unused-function").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_unused_function_json_empty() {
        let json = r#"{
  "functions": []
}"#;

        let receipt = convert_unused_function_json(json, "cargo-unused-function").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }
}
