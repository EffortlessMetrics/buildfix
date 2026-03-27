use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoAuditAdapter {
    sensor_id: String,
}

impl CargoAuditAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-audit".to_string(),
        }
    }
}

impl Default for CargoAuditAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoAuditAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_audit_json(&content, &self.sensor_id)
    }
}

impl AdapterMetadata for CargoAuditAdapter {
    fn name(&self) -> &str {
        "cargo-audit"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-audit.report.v1"]
    }
}

fn convert_cargo_audit_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let report: CargoAuditReport = serde_json::from_str(content).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();
    let mut error_count = 0u64;
    let mut warning_count = 0u64;
    let mut info_count = 0u64;

    if let Some(advisories) = &report.cargo_audit.advisories {
        for advisory in advisories {
            let severity = map_severity(&advisory.severity);
            match severity {
                Severity::Error => error_count += 1,
                Severity::Warn => warning_count += 1,
                Severity::Info => info_count += 1,
            }

            let check_id = format!("advisory.{}", advisory.id);

            let message = format!("{}\n{}", advisory.title, advisory.description);

            let location = Location {
                path: Utf8PathBuf::from("Cargo.toml"),
                line: None,
                column: None,
            };

            let data = CargoAuditAdvisoryData {
                id: advisory.id.clone(),
                package_name: advisory.package.name.clone(),
                package_version: advisory.package.version.clone(),
                date: advisory.date.clone(),
                categories: advisory.categories.clone(),
                url: advisory.url.clone(),
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

    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else if warning_count > 0 || info_count > 0 || !findings.is_empty() {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-audit.report.v1")
        .with_tool_version(report.cargo_audit.version.clone())
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, warning_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

fn map_severity(severity: &Option<String>) -> Severity {
    match severity.as_deref() {
        Some("High") => Severity::Error,
        Some("Medium") => Severity::Warn,
        Some("Low") => Severity::Info,
        _ => Severity::Warn,
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CargoAuditReport {
    #[serde(rename = "cargo_audit")]
    cargo_audit: CargoAuditData,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CargoAuditData {
    version: String,
    advisories: Option<Vec<Advisory>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Advisory {
    id: String,
    #[serde(rename = "package")]
    package: Package,
    title: String,
    #[serde(rename = "date")]
    date: Option<String>,
    description: String,
    severity: Option<String>,
    categories: Option<Vec<String>>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Package {
    name: String,
    version: String,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct CargoAuditAdvisoryData {
    id: String,
    package_name: String,
    package_version: String,
    date: Option<String>,
    categories: Option<Vec<String>>,
    url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoAuditAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-audit");
    }

    #[test]
    fn test_convert_cargo_audit_json_with_advisories() {
        let json = r#"{
  "cargo_audit": {
    "version": "0.18.0",
    "advisories": [
      {
        "id": "RUSTSEC-0001-0001",
        "package": {
          "name": "vulnerable-crate",
          "version": "1.0.0"
        },
        "title": "Vulnerability in vulnerable-crate",
        "date": "2023-01-01",
        "description": "Description of vulnerability",
        "severity": "High",
        "categories": ["security"],
        "url": "https://rustsec.org/advisories/RUSTSEC-0001-0001"
      }
    ]
  }
}"#;

        let receipt = convert_cargo_audit_json(json, "cargo-audit").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Error);
        assert_eq!(
            finding.check_id,
            Some("advisory.RUSTSEC-0001-0001".to_string())
        );
        assert!(
            finding
                .message
                .as_ref()
                .unwrap()
                .contains("Vulnerability in vulnerable-crate")
        );
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "Cargo.toml"
        );
    }

    #[test]
    fn test_convert_cargo_audit_json_maps_severity() {
        let json = r#"{
  "cargo_audit": {
    "version": "0.18.0",
    "advisories": [
      {
        "id": "RUSTSEC-0001-0001",
        "package": {"name": "crate1", "version": "1.0.0"},
        "title": "High severity",
        "description": "Desc",
        "severity": "High",
        "categories": [],
        "url": null
      },
      {
        "id": "RUSTSEC-0002-0002",
        "package": {"name": "crate2", "version": "1.0.0"},
        "title": "Medium severity",
        "description": "Desc",
        "severity": "Medium",
        "categories": [],
        "url": null
      },
      {
        "id": "RUSTSEC-0003-0003",
        "package": {"name": "crate3", "version": "1.0.0"},
        "title": "Low severity",
        "description": "Desc",
        "severity": "Low",
        "categories": [],
        "url": null
      },
      {
        "id": "RUSTSEC-0004-0004",
        "package": {"name": "crate4", "version": "1.0.0"},
        "title": "Unknown severity",
        "description": "Desc",
        "severity": null,
        "categories": [],
        "url": null
      }
    ]
  }
}"#;

        let receipt = convert_cargo_audit_json(json, "cargo-audit").unwrap();

        assert_eq!(receipt.findings.len(), 4);
        assert_eq!(receipt.findings[0].severity, Severity::Error);
        assert_eq!(receipt.findings[1].severity, Severity::Warn);
        assert_eq!(receipt.findings[2].severity, Severity::Info);
        assert_eq!(receipt.findings[3].severity, Severity::Warn);
    }

    #[test]
    fn test_convert_cargo_audit_json_calculates_counts() {
        let json = r#"{
  "cargo_audit": {
    "version": "0.18.0",
    "advisories": [
      {
        "id": "RUSTSEC-0001-0001",
        "package": {"name": "crate1", "version": "1.0.0"},
        "title": "High 1",
        "description": "Desc",
        "severity": "High",
        "categories": [],
        "url": null
      },
      {
        "id": "RUSTSEC-0002-0002",
        "package": {"name": "crate2", "version": "1.0.0"},
        "title": "High 2",
        "description": "Desc",
        "severity": "High",
        "categories": [],
        "url": null
      },
      {
        "id": "RUSTSEC-0003-0003",
        "package": {"name": "crate3", "version": "1.0.0"},
        "title": "Medium",
        "description": "Desc",
        "severity": "Medium",
        "categories": [],
        "url": null
      }
    ]
  }
}"#;

        let receipt = convert_cargo_audit_json(json, "cargo-audit").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.findings, 3);
        assert_eq!(receipt.verdict.counts.errors, 2);
        assert_eq!(receipt.verdict.counts.warnings, 1);
    }

    #[test]
    fn test_convert_cargo_audit_json_empty_passes() {
        let json = r#"{
  "cargo_audit": {
    "version": "0.18.0",
    "advisories": []
  }
}"#;

        let receipt = convert_cargo_audit_json(json, "cargo-audit").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_audit_json_no_advisories_passes() {
        let json = r#"{
  "cargo_audit": {
    "version": "0.18.0"
  }
}"#;

        let receipt = convert_cargo_audit_json(json, "cargo-audit").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }
}
