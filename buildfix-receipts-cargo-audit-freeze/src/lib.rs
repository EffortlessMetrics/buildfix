use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoAuditFreezeAdapter {
    sensor_id: String,
}

impl CargoAuditFreezeAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-audit-freeze".to_string(),
        }
    }
}

impl Default for CargoAuditFreezeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoAuditFreezeAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_audit_freeze_json(&content, &self.sensor_id)
    }
}

fn convert_cargo_audit_freeze_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let report: CargoAuditFreezeReport =
        serde_json::from_str(content).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();

    if let Some(frozen) = &report.frozen {
        for pkg in frozen {
            let check_id = "deps.pinned".to_string();
            let message = format!(
                "Dependency {} is pinned to version {}",
                pkg.name, pkg.version
            );

            let location = Location {
                path: Utf8PathBuf::from("Cargo.lock"),
                line: None,
                column: None,
            };

            let data = CargoAuditFreezeData {
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                locked_version: pkg.locked_version.clone(),
            };

            findings.push(Finding {
                severity: Severity::Info,
                check_id: Some(check_id),
                code: None,
                message: Some(message),
                location: Some(location),
                fingerprint: None,
                data: Some(serde_json::to_value(data).unwrap_or_default()),
            });
        }
    }

    let status = if findings.is_empty() {
        VerdictStatus::Pass
    } else {
        VerdictStatus::Warn
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-audit-freeze.report.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, 0);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CargoAuditFreezeReport {
    frozen: Option<Vec<FrozenPackage>>,
    #[serde(rename = "updates_available")]
    updates_available: Option<Vec<UpdatePackage>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FrozenPackage {
    name: String,
    version: String,
    #[serde(rename = "locked_version")]
    locked_version: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct UpdatePackage {
    name: String,
    version: String,
    #[serde(rename = "locked_version")]
    locked_version: String,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct CargoAuditFreezeData {
    name: String,
    version: String,
    locked_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoAuditFreezeAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-audit-freeze");
    }

    #[test]
    fn test_convert_cargo_audit_freeze_json_with_frozen() {
        let json = r#"{
  "frozen": [
    {
      "name": "serde",
      "version": "1.0.200",
      "locked_version": "1.0.200"
    },
    {
      "name": "tokio",
      "version": "1.40.0",
      "locked_version": "1.40.0"
    }
  ],
  "updates_available": []
}"#;

        let receipt = convert_cargo_audit_freeze_json(json, "cargo-audit-freeze").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Info);
        assert_eq!(finding.check_id, Some("deps.pinned".to_string()));
        assert!(finding.message.as_ref().unwrap().contains("serde"));
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "Cargo.lock"
        );
    }

    #[test]
    fn test_convert_cargo_audit_freeze_json_empty_passes() {
        let json = r#"{
  "frozen": [],
  "updates_available": []
}"#;

        let receipt = convert_cargo_audit_freeze_json(json, "cargo-audit-freeze").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_audit_freeze_json_no_frozen() {
        let json = r#"{
  "frozen": null,
  "updates_available": []
}"#;

        let receipt = convert_cargo_audit_freeze_json(json, "cargo-audit-freeze").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_audit_freeze_json_calculates_counts() {
        let json = r#"{
  "frozen": [
    {
      "name": "crate1",
      "version": "1.0.0",
      "locked_version": "1.0.0"
    },
    {
      "name": "crate2",
      "version": "2.0.0",
      "locked_version": "2.0.0"
    }
  ],
  "updates_available": []
}"#;

        let receipt = convert_cargo_audit_freeze_json(json, "cargo-audit-freeze").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.errors, 0);
        assert_eq!(receipt.verdict.counts.warnings, 0);
    }
}
