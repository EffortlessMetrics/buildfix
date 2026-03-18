use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct RustfmtAdapter {
    sensor_id: String,
}

impl RustfmtAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "rustfmt".to_string(),
        }
    }
}

impl Default for RustfmtAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for RustfmtAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_rustfmt_json(&content, &self.sensor_id)
    }
}

fn convert_rustfmt_json(content: &str, sensor_id: &str) -> Result<ReceiptEnvelope, AdapterError> {
    let report: RustfmtReport = serde_json::from_str(content).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();
    let mut warning_count = 0u64;

    for file in &report.files {
        for error in &file.errors {
            warning_count += 1;

            let check_id = if error.kind.contains("Formatting") {
                "rustfmt.format".to_string()
            } else {
                "formatting.mismatch".to_string()
            };

            findings.push(Finding {
                severity: Severity::Warn,
                check_id: Some(check_id),
                code: None,
                message: Some(error.message.clone()),
                location: Some(Location {
                    path: Utf8PathBuf::from(&file.name),
                    line: None,
                    column: None,
                }),
                fingerprint: None,
                data: None,
            });
        }
    }

    let status = if warning_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("rustfmt.check.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, warning_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct RustfmtReport {
    #[serde(default)]
    files: Vec<RustfmtFile>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct RustfmtFile {
    #[serde(default)]
    name: String,
    #[serde(default)]
    errors: Vec<RustfmtError>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct RustfmtError {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = RustfmtAdapter::new();
        assert_eq!(adapter.sensor_id(), "rustfmt");
    }

    #[test]
    fn test_convert_rustfmt_json_with_errors() {
        let json = r#"{
  "files": [
    {
      "name": "src/lib.rs",
      "errors": [
        {
          "kind": "Formatting",
          "message": "mismatched formatting found"
        }
      ]
    }
  ]
}
"#;

        let receipt = convert_rustfmt_json(json, "rustfmt").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(finding.check_id, Some("rustfmt.format".to_string()));
        assert_eq!(
            finding.message,
            Some("mismatched formatting found".to_string())
        );
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "src/lib.rs"
        );
    }

    #[test]
    fn test_convert_rustfmt_json_multiple_files() {
        let json = r#"{
  "files": [
    {
      "name": "src/lib.rs",
      "errors": [
        {
          "kind": "Formatting",
          "message": "mismatched formatting found"
        }
      ]
    },
    {
      "name": "src/main.rs",
      "errors": [
        {
          "kind": "Formatting",
          "message": "different formatting found"
        }
      ]
    }
  ]
}
"#;

        let receipt = convert_rustfmt_json(json, "rustfmt").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_rustfmt_json_no_errors() {
        let json = r#"{
  "files": []
}
"#;

        let receipt = convert_rustfmt_json(json, "rustfmt").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_rustfmt_json_check_id_fallback() {
        let json = r#"{
  "files": [
    {
      "name": "src/lib.rs",
      "errors": [
        {
          "kind": "OtherError",
          "message": "some error"
        }
      ]
    }
  ]
}
"#;

        let receipt = convert_rustfmt_json(json, "rustfmt").unwrap();

        assert_eq!(
            receipt.findings[0].check_id,
            Some("formatting.mismatch".to_string())
        );
    }
}
