use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct CargoUdepsAdapter;

impl CargoUdepsAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoUdepsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AdapterMetadata for CargoUdepsAdapter {
    fn name(&self) -> &str {
        "cargo-udeps"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-udeps.report.v1"]
    }
}

impl Adapter for CargoUdepsAdapter {
    fn sensor_id(&self) -> &str {
        "cargo-udeps"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: CargoUdepsReport =
            serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

fn convert_report(report: CargoUdepsReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut warn_count = 0u64;

    if let Some(packages) = &report.packages {
        for package in packages {
            let check_id = match package.kind.first().map(|k| k.as_str()).unwrap_or("Normal") {
                "Dev" => "deps.unused_dependency",
                "Build" => "deps.unused_build_dependency",
                _ => "deps.unused_dependency",
            };

            let location = Location {
                path: Utf8PathBuf::from(&package.manifest_path),
                line: None,
                column: None,
            };

            let message = format!("unused {}:{}", package.name, package.version);

            let data = UdepsPackageData {
                name: package.name.clone(),
                version: package.version.clone(),
                edition: package.edition.clone(),
                kind: package.kind.clone(),
            };

            findings.push(Finding {
                severity: Severity::Warn,
                check_id: Some(check_id.to_string()),
                code: None,
                message: Some(message),
                location: Some(location),
                fingerprint: None,
                data: Some(serde_json::to_value(data).unwrap_or_default()),
                ..Default::default()
            });

            warn_count += 1;
        }
    }

    let status = if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new("cargo-udeps")
        .with_schema("cargo-udeps.report.v1")
        .with_tool_version("0.0.0")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, warn_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    let receipt = builder.build();

    Ok(receipt)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CargoUdepsReport {
    success: bool,
    #[serde(default)]
    packages: Option<Vec<UdepsPackage>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UdepsPackage {
    manifest_path: String,
    name: String,
    version: String,
    edition: Option<String>,
    kind: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UdepsPackageData {
    name: String,
    version: String,
    edition: Option<String>,
    kind: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoUdepsAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-udeps");
    }

    #[test]
    fn test_convert_report_with_unused_packages() {
        let json = r#"{
            "success": true,
            "packages": [
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "name": "unused-crate",
                    "version": "0.1.0",
                    "edition": "2021",
                    "kind": ["Normal"]
                },
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "name": "unused-dev-dep",
                    "version": "0.2.0",
                    "edition": "2021",
                    "kind": ["Dev"]
                }
            ]
        }"#;

        let report: CargoUdepsReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 2);

        let finding1 = &receipt.findings[0];
        assert_eq!(finding1.severity, Severity::Warn);
        assert_eq!(
            finding1.check_id,
            Some("deps.unused_dependency".to_string())
        );

        let finding2 = &receipt.findings[1];
        assert_eq!(finding2.severity, Severity::Warn);
        assert_eq!(
            finding2.check_id,
            Some("deps.unused_dependency".to_string())
        );

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_report_empty_passes() {
        let json = r#"{
            "success": true,
            "packages": []
        }"#;

        let report: CargoUdepsReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_no_packages_passes() {
        let json = r#"{
            "success": true
        }"#;

        let report: CargoUdepsReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_dev_dependency_check_id() {
        let json = r#"{
            "success": true,
            "packages": [
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "name": "some-dev-dep",
                    "version": "1.0.0",
                    "edition": "2021",
                    "kind": ["Dev"]
                }
            ]
        }"#;

        let report: CargoUdepsReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(
            receipt.findings[0].check_id,
            Some("deps.unused_dependency".to_string())
        );
    }

    #[test]
    fn test_convert_report_build_dependency_check_id() {
        let json = r#"{
            "success": true,
            "packages": [
                {
                    "manifestPath": "/path/to/Cargo.toml",
                    "name": "some-build-dep",
                    "version": "1.0.0",
                    "edition": "2021",
                    "kind": ["Build"]
                }
            ]
        }"#;

        let report: CargoUdepsReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(
            receipt.findings[0].check_id,
            Some("deps.unused_build_dependency".to_string())
        );
    }
}
