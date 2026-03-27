use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoCrevAdapter {
    sensor_id: String,
}

impl CargoCrevAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-crev".to_string(),
        }
    }
}

impl Default for CargoCrevAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoCrevAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_crev_json(&content, &self.sensor_id)
    }
}

impl AdapterMetadata for CargoCrevAdapter {
    fn name(&self) -> &str {
        "cargo-crev"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-crev.report.v1"]
    }
}

fn convert_cargo_crev_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let report: CrevReport = serde_json::from_str(content).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();
    let mut error_count = 0u64;
    let mut warning_count = 0u64;
    let mut info_count = 0u64;

    if let Some(reviews) = &report.reviews {
        for review in reviews {
            for issue in &review.issues {
                let severity = map_severity(&issue.severity);
                match severity {
                    Severity::Error => error_count += 1,
                    Severity::Warn => warning_count += 1,
                    Severity::Info => info_count += 1,
                };

                let check_id = format!("review.{}", issue.title.to_lowercase().replace(' ', "_"));

                let message = format!("{}\n{}", issue.title, issue.description);

                let location = Location {
                    path: Utf8PathBuf::from("Cargo.toml"),
                    line: None,
                    column: None,
                };

                let data = CrevIssueData {
                    package_name: review.package.clone(),
                    package_version: review.version.clone(),
                    reviews_count: review.reviews_count,
                    issue_title: issue.title.clone(),
                    issue_description: issue.description.clone(),
                };

                findings.push(Finding {
                    severity,
                    check_id: Some(check_id),
                    code: None,
                    message: Some(message),
                    location: Some(location),
                    fingerprint: None,
                    data: Some(serde_json::to_value(data).unwrap_or_default()),
                    ..Default::default()
                });
            }
        }
    }

    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else if warning_count > 0 || info_count > 0 || !findings.is_empty() {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-crev.report.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, warning_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

fn map_severity(severity: &str) -> Severity {
    match severity.to_lowercase().as_str() {
        "high" => Severity::Error,
        "medium" => Severity::Warn,
        "low" => Severity::Info,
        _ => Severity::Warn,
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CrevReport {
    reviews: Option<Vec<CrevReview>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CrevReview {
    package: String,
    version: String,
    #[serde(rename = "reviews_count")]
    reviews_count: u64,
    issues: Vec<CrevIssue>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CrevIssue {
    severity: String,
    title: String,
    description: String,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct CrevIssueData {
    package_name: String,
    package_version: String,
    reviews_count: u64,
    issue_title: String,
    issue_description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoCrevAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-crev");
    }

    #[test]
    fn test_convert_cargo_crev_json_with_reviews() {
        let json = r#"{
  "reviews": [
    {
      "package": "some-crate",
      "version": "1.0.0",
      "reviews_count": 0,
      "issues": [
        {
          "severity": "high",
          "title": "No reviews",
          "description": "This crate has no trusted reviews"
        }
      ]
    }
  ]
}"#;

        let receipt = convert_cargo_crev_json(json, "cargo-crev").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Error);
        assert_eq!(finding.check_id, Some("review.no_reviews".to_string()));
        assert!(finding.message.as_ref().unwrap().contains("No reviews"));
    }

    #[test]
    fn test_convert_cargo_crev_json_maps_severity() {
        let json = r#"{
  "reviews": [
    {
      "package": "crate1",
      "version": "1.0.0",
      "reviews_count": 0,
      "issues": [{"severity": "high", "title": "High issue", "description": "Desc"}]
    },
    {
      "package": "crate2",
      "version": "1.0.0",
      "reviews_count": 0,
      "issues": [{"severity": "medium", "title": "Medium issue", "description": "Desc"}]
    },
    {
      "package": "crate3",
      "version": "1.0.0",
      "reviews_count": 0,
      "issues": [{"severity": "low", "title": "Low issue", "description": "Desc"}]
    }
  ]
}"#;

        let receipt = convert_cargo_crev_json(json, "cargo-crev").unwrap();

        assert_eq!(receipt.findings.len(), 3);
        assert_eq!(receipt.findings[0].severity, Severity::Error);
        assert_eq!(receipt.findings[1].severity, Severity::Warn);
        assert_eq!(receipt.findings[2].severity, Severity::Info);
    }

    #[test]
    fn test_convert_cargo_crev_json_calculates_counts() {
        let json = r#"{
  "reviews": [
    {
      "package": "crate1",
      "version": "1.0.0",
      "reviews_count": 0,
      "issues": [
        {"severity": "high", "title": "High 1", "description": "Desc"},
        {"severity": "high", "title": "High 2", "description": "Desc"}
      ]
    },
    {
      "package": "crate2",
      "version": "1.0.0",
      "reviews_count": 0,
      "issues": [{"severity": "medium", "title": "Medium", "description": "Desc"}]
    }
  ]
}"#;

        let receipt = convert_cargo_crev_json(json, "cargo-crev").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.findings, 3);
        assert_eq!(receipt.verdict.counts.errors, 2);
        assert_eq!(receipt.verdict.counts.warnings, 1);
    }

    #[test]
    fn test_convert_cargo_crev_json_empty_passes() {
        let json = r#"{
  "reviews": []
}"#;

        let receipt = convert_cargo_crev_json(json, "cargo-crev").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_crev_json_no_reviews_passes() {
        let json = r#"{}"#;

        let receipt = convert_cargo_crev_json(json, "cargo-crev").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }
}
