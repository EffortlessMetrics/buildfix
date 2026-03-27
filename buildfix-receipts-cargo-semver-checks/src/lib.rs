use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, ReceiptEnvelope, Severity, VerdictStatus};
use serde::Deserialize;
use std::path::Path;

pub struct CargoSemverChecksAdapter {
    sensor_id: String,
}

impl CargoSemverChecksAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-semver-checks".to_string(),
        }
    }
}

impl Default for CargoSemverChecksAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoSemverChecksAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_semver_json(&content, &self.sensor_id)
    }
}

impl AdapterMetadata for CargoSemverChecksAdapter {
    fn name(&self) -> &str {
        "cargo-semver-checks"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-semver-checks.report.v1"]
    }
}

fn convert_semver_json(content: &str, sensor_id: &str) -> Result<ReceiptEnvelope, AdapterError> {
    let report: SemverReport =
        serde_json::from_str(content).map_err(|e| AdapterError::InvalidFormat(e.to_string()))?;

    let mut findings = Vec::new();
    let mut error_count = 0u64;

    for check in &report.semver_checks {
        for error in &check.errors {
            error_count += 1;

            let check_id = format!("semver.{}", error.code.replace("-", "."));

            let message = if let Some(details) = &error.details {
                format!("{}: {}", error.message, details)
            } else {
                error.message.clone()
            };

            findings.push(Finding {
                severity: Severity::Error,
                check_id: Some(check_id),
                code: Some(error.code.clone()),
                message: Some(message),
                location: None,
                fingerprint: None,
                data: None,
                ..Default::default()
            });
        }
    }

    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-semver-checks.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, 0);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct SemverReport {
    #[serde(default)]
    semver_checks: Vec<SemverCheck>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct SemverCheck {
    #[serde(default)]
    package: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    errors: Vec<SemverError>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct SemverError {
    #[serde(default)]
    code: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    details: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoSemverChecksAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-semver-checks");
    }

    #[test]
    fn test_convert_semver_json_with_errors() {
        let json = r#"{
  "semver_checks": [
    {
      "package": "my-crate",
      "version": "1.0.0",
      "errors": [
        {
          "code": "RUSTSEMVER-CRATE-1.0.0",
          "message": "breaking change: removed public function",
          "details": "function foo was removed"
        }
      ]
    }
  ]
}
"#;

        let receipt = convert_semver_json(json, "cargo-semver-checks").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Error);
        assert_eq!(
            finding.check_id,
            Some("semver.RUSTSEMVER.CRATE.1.0.0".to_string())
        );
        assert!(
            finding
                .message
                .as_ref()
                .unwrap()
                .contains("breaking change")
        );
    }

    #[test]
    fn test_convert_semver_json_multiple_errors() {
        let json = r#"{
  "semver_checks": [
    {
      "package": "my-crate",
      "version": "1.0.0",
      "errors": [
        {
          "code": "RUSTSEMVER-CRATE-1.0.0",
          "message": "error 1",
          "details": null
        },
        {
          "code": "RUSTSEMVER-CRATE-1.0.1",
          "message": "error 2",
          "details": "details"
        }
      ]
    }
  ]
}
"#;

        let receipt = convert_semver_json(json, "cargo-semver-checks").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.errors, 2);
    }

    #[test]
    fn test_convert_semver_json_empty_passes() {
        let json = r#"{"semver_checks": []}
"#;

        let receipt = convert_semver_json(json, "cargo-semver-checks").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_semver_json_multiple_packages() {
        let json = r#"{
  "semver_checks": [
    {
      "package": "crate-a",
      "version": "1.0.0",
      "errors": [
        {
          "code": "RUSTSEMVER-CRATE-1.0.0",
          "message": "error in crate-a",
          "details": null
        }
      ]
    },
    {
      "package": "crate-b",
      "version": "2.0.0",
      "errors": [
        {
          "code": "RUSTSEMVER-CRATE-2.0.0",
          "message": "error in crate-b",
          "details": null
        }
      ]
    }
  ]
}
"#;

        let receipt = convert_semver_json(json, "cargo-semver-checks").unwrap();

        assert_eq!(receipt.findings.len(), 2);
    }

    #[test]
    fn test_convert_semver_json_check_id_format() {
        let json = r#"{
  "semver_checks": [
    {
      "package": "my-crate",
      "version": "1.0.0",
      "errors": [
        {
          "code": "RUSTSEMVER-PACKAGE-1.0.0",
          "message": "breaking change",
          "details": null
        }
      ]
    }
  ]
}
"#;

        let receipt = convert_semver_json(json, "cargo-semver-checks").unwrap();

        assert_eq!(
            receipt.findings[0].check_id,
            Some("semver.RUSTSEMVER.PACKAGE.1.0.0".to_string())
        );
    }
}
