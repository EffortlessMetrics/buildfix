use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoSecAuditAdapter {
    sensor_id: String,
}

impl CargoSecAuditAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-sec-audit".to_string(),
        }
    }
}

impl Default for CargoSecAuditAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoSecAuditAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_sec_audit_json(&content, &self.sensor_id)
    }
}

fn convert_cargo_sec_audit_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let report: CargoSecAuditReport = serde_json::from_str(content).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();
    let mut error_count = 0u64;
    let mut warning_count = 0u64;
    let mut info_count = 0u64;

    if let Some(vulnerabilities) = &report.vulnerabilities {
        for vuln in vulnerabilities {
            let severity = map_severity(&vuln.severity);
            match severity {
                Severity::Error => error_count += 1,
                Severity::Warn => warning_count += 1,
                Severity::Info => info_count += 1,
            }

            let check_id = format!("security.{}", vuln.id);

            let message = format!("{}\n{}", vuln.title, vuln.description);

            let location = Location {
                path: Utf8PathBuf::from("Cargo.toml"),
                line: None,
                column: None,
            };

            let data = CargoSecAuditVulnData {
                id: vuln.id.clone(),
                package_name: vuln.package.clone(),
                package_version: vuln.version.clone(),
                date: vuln.date.clone(),
                url: vuln.url.clone(),
            };

            findings.push(Finding {
                severity,
                check_id: Some(check_id),
                code: None,
                message: Some(message),
                location: Some(location),
                fingerprint: None,
                data: Some(serde_json::to_value(data).unwrap_or_default()),
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
        .with_schema("cargo-sec-audit.report.v1")
        .with_tool_version(report.audit_version.clone())
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, warning_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

fn map_severity(severity: &Option<String>) -> Severity {
    match severity.as_deref() {
        Some("HIGH") => Severity::Error,
        Some("MEDIUM") => Severity::Warn,
        Some("LOW") => Severity::Info,
        _ => Severity::Warn,
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CargoSecAuditReport {
    #[serde(rename = "audit_version")]
    audit_version: String,
    vulnerabilities: Option<Vec<Vulnerability>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Vulnerability {
    id: String,
    package: String,
    version: String,
    title: String,
    description: String,
    date: Option<String>,
    severity: Option<String>,
    url: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct CargoSecAuditVulnData {
    id: String,
    package_name: String,
    package_version: String,
    date: Option<String>,
    url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoSecAuditAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-sec-audit");
    }

    #[test]
    fn test_convert_cargo_sec_audit_json_with_vulnerabilities() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": [
    {
      "id": "RUSTSEC-0001-0001",
      "package": "vulnerable-crate",
      "version": "1.0.0",
      "title": "Vulnerability in vulnerable-crate",
      "description": "Description of the vulnerability",
      "date": "2023-01-01",
      "severity": "HIGH",
      "url": "https://rustsec.org/advisories/RUSTSEC-0001-0001"
    }
  ]
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Error);
        assert_eq!(
            finding.check_id,
            Some("security.RUSTSEC-0001-0001".to_string())
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
    fn test_convert_cargo_sec_audit_json_maps_severity() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": [
    {
      "id": "RUSTSEC-0001-0001",
      "package": "crate1",
      "version": "1.0.0",
      "title": "High severity",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": "HIGH",
      "url": null
    },
    {
      "id": "RUSTSEC-0002-0002",
      "package": "crate2",
      "version": "1.0.0",
      "title": "Medium severity",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": "MEDIUM",
      "url": null
    },
    {
      "id": "RUSTSEC-0003-0003",
      "package": "crate3",
      "version": "1.0.0",
      "title": "Low severity",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": "LOW",
      "url": null
    },
    {
      "id": "RUSTSEC-0004-0004",
      "package": "crate4",
      "version": "1.0.0",
      "title": "Unknown severity",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": null,
      "url": null
    }
  ]
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();

        assert_eq!(receipt.findings.len(), 4);
        assert_eq!(receipt.findings[0].severity, Severity::Error);
        assert_eq!(receipt.findings[1].severity, Severity::Warn);
        assert_eq!(receipt.findings[2].severity, Severity::Info);
        assert_eq!(receipt.findings[3].severity, Severity::Warn);
    }

    #[test]
    fn test_convert_cargo_sec_audit_json_calculates_counts() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": [
    {
      "id": "RUSTSEC-0001-0001",
      "package": "crate1",
      "version": "1.0.0",
      "title": "High 1",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": "HIGH",
      "url": null
    },
    {
      "id": "RUSTSEC-0002-0002",
      "package": "crate2",
      "version": "1.0.0",
      "title": "High 2",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": "HIGH",
      "url": null
    },
    {
      "id": "RUSTSEC-0003-0003",
      "package": "crate3",
      "version": "1.0.0",
      "title": "Medium",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": "MEDIUM",
      "url": null
    }
  ]
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.findings, 3);
        assert_eq!(receipt.verdict.counts.errors, 2);
        assert_eq!(receipt.verdict.counts.warnings, 1);
    }

    #[test]
    fn test_convert_cargo_sec_audit_json_empty_passes() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": []
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_sec_audit_json_no_vulnerabilities_passes() {
        let json = r#"{
  "audit_version": "0.1.0"
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_adapter_loads_from_file() {
        let adapter = CargoSecAuditAdapter::new();
        let receipt = adapter
            .load(Path::new("tests/fixtures/report.json"))
            .expect("should load fixture");

        assert_eq!(adapter.sensor_id(), "cargo-sec-audit");
        assert!(!receipt.findings.is_empty());
    }

    #[test]
    fn test_adapter_returns_error_for_missing_file() {
        let adapter = CargoSecAuditAdapter::new();
        let result = adapter.load(Path::new("nonexistent/path.json"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Io(_)));
    }

    #[test]
    fn test_adapter_returns_error_for_invalid_json() {
        let adapter = CargoSecAuditAdapter::new();

        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().join("invalid.json");
        std::fs::write(&temp_path, "{ invalid json }").unwrap();

        let result = adapter.load(&temp_path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Json(_)));
    }

    #[test]
    fn test_map_severity_with_null() {
        let severity_none: Option<String> = None;
        assert_eq!(map_severity(&severity_none), Severity::Warn);
    }

    #[test]
    fn test_map_severity_case_sensitivity() {
        assert_eq!(map_severity(&Some("high".to_string())), Severity::Warn);
        assert_eq!(map_severity(&Some("Medium".to_string())), Severity::Warn);
        assert_eq!(map_severity(&Some("LOW".to_string())), Severity::Info);
    }

    #[test]
    fn test_verdict_fail_when_errors_present() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": [
    {
      "id": "RUSTSEC-0001-0001",
      "package": "crate1",
      "version": "1.0.0",
      "title": "High severity",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": "HIGH",
      "url": null
    }
  ]
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();
        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
    }

    #[test]
    fn test_verdict_warn_when_only_warnings() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": [
    {
      "id": "RUSTSEC-0001-0001",
      "package": "crate1",
      "version": "1.0.0",
      "title": "Medium severity",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": "MEDIUM",
      "url": null
    }
  ]
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }

    #[test]
    fn test_verdict_warn_when_only_info() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": [
    {
      "id": "RUSTSEC-0001-0001",
      "package": "crate1",
      "version": "1.0.0",
      "title": "Low severity",
      "description": "Desc",
      "date": "2023-01-01",
      "severity": "LOW",
      "url": null
    }
  ]
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }

    #[test]
    fn test_finding_data_contains_vuln_info() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": [
    {
      "id": "RUSTSEC-0001-0001",
      "package": "test-crate",
      "version": "1.2.3",
      "title": "Test vulnerability",
      "description": "Test description",
      "date": "2023-01-01",
      "severity": "HIGH",
      "url": "https://rustsec.org/advisories/RUSTSEC-0001-0001"
    }
  ]
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();

        let finding = &receipt.findings[0];
        let data = finding.data.as_ref().unwrap();
        assert!(data.get("id").is_some());
        assert!(data.get("packageName").is_some() || data.get("package_name").is_some());
        assert!(data.get("packageVersion").is_some() || data.get("package_version").is_some());
    }

    #[test]
    fn test_convert_with_null_severity() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": [
    {
      "id": "RUSTSEC-0001-0001",
      "package": "crate1",
      "version": "1.0.0",
      "title": "Unknown severity",
      "description": "Desc",
      "date": null,
      "severity": null,
      "url": null
    }
  ]
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Warn);
    }

    #[test]
    fn test_counts_with_mixed_severities() {
        let json = r#"{
  "audit_version": "0.1.0",
  "vulnerabilities": [
    {
      "id": "RUSTSEC-0001-0001",
      "package": "crate1",
      "version": "1.0.0",
      "title": "High",
      "description": "Desc",
      "severity": "HIGH"
    },
    {
      "id": "RUSTSEC-0002-0002",
      "package": "crate2",
      "version": "1.0.0",
      "title": "Low",
      "description": "Desc",
      "severity": "LOW"
    },
    {
      "id": "RUSTSEC-0003-0003",
      "package": "crate3",
      "version": "1.0.0",
      "title": "Medium",
      "description": "Desc",
      "severity": "MEDIUM"
    }
  ]
}"#;

        let receipt = convert_cargo_sec_audit_json(json, "cargo-sec-audit").unwrap();

        assert_eq!(receipt.verdict.counts.findings, 3);
        assert_eq!(receipt.verdict.counts.errors, 1);
        assert_eq!(receipt.verdict.counts.warnings, 1);
    }
}
