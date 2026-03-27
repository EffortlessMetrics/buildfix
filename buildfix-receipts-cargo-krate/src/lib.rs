use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoKrateAdapter {
    sensor_id: String,
}

impl CargoKrateAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-krate".to_string(),
        }
    }
}

impl Default for CargoKrateAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoKrateAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_krate_json(&content, &self.sensor_id)
    }
}

impl AdapterMetadata for CargoKrateAdapter {
    fn name(&self) -> &str {
        "cargo-krate"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-krate.report.v1"]
    }
}

fn convert_cargo_krate_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let parsed: serde_json::Value = serde_json::from_str(content).map_err(AdapterError::Json)?;

    let report: KrateReport = serde_json::from_value(parsed.clone()).map_err(AdapterError::Json)?;

    let crate_count = report.crates.len() as u64;

    let mut findings = Vec::new();

    for krate in &report.crates {
        let message = format!("{} v{}", krate.name, krate.version);

        let data = KrateData {
            name: krate.name.clone(),
            version: krate.version.clone(),
            description: krate.description.clone(),
            downloads: krate.downloads,
            recent_downloads: krate.recent_downloads,
            repository: krate.repository.clone(),
            license: krate.license.clone(),
            categories: krate.categories.clone(),
            keywords: krate.keywords.clone(),
        };

        findings.push(Finding {
            severity: Severity::Info,
            check_id: Some("metadata.crate_info".to_string()),
            code: None,
            message: Some(message),
            location: Some(Location {
                path: Utf8PathBuf::from("Cargo.toml"),
                line: None,
                column: None,
            }),
            fingerprint: None,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            ..Default::default()
        });
    }

    let builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-krate.report.v1")
        .with_status(VerdictStatus::Pass)
        .with_counts(crate_count, 0, 0);

    let mut receipt = builder.build();
    receipt.data = Some(serde_json::json!({
        "crate_count": crate_count,
    }));

    for finding in findings {
        receipt.findings.push(finding);
    }

    Ok(receipt)
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct KrateReport {
    crates: Vec<Crate>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Crate {
    name: String,
    version: String,
    description: Option<String>,
    downloads: Option<u64>,
    recent_downloads: Option<u64>,
    repository: Option<String>,
    license: Option<String>,
    categories: Option<Vec<String>>,
    keywords: Option<Vec<String>>,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct KrateData {
    name: String,
    version: String,
    description: Option<String>,
    downloads: Option<u64>,
    recent_downloads: Option<u64>,
    repository: Option<String>,
    license: Option<String>,
    categories: Option<Vec<String>>,
    keywords: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoKrateAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-krate");
    }

    #[test]
    fn test_convert_cargo_krate_json_basic() {
        let json = r#"{
          "crates": [
            {
              "name": "serde",
              "version": "1.0.0",
              "description": "Serialization framework",
              "downloads": 1000000,
              "recent_downloads": 10000,
              "repository": "https://github.com/serde-rs/serde",
              "license": "MIT OR Apache-2.0",
              "categories": ["encoding", "serialization"],
              "keywords": ["serde", "serialization"]
            }
          ]
        }"#;

        let receipt = convert_cargo_krate_json(json, "cargo-krate").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Info);
        assert_eq!(finding.check_id, Some("metadata.crate_info".to_string()));
        assert!(finding.message.as_ref().unwrap().contains("serde"));
        assert!(finding.message.as_ref().unwrap().contains("1.0.0"));
    }

    #[test]
    fn test_convert_cargo_krate_json_multiple_crates() {
        let json = r#"{
          "crates": [
            {
              "name": "serde",
              "version": "1.0.0",
              "description": "Serialization framework",
              "downloads": 1000000,
              "recent_downloads": 10000,
              "repository": "https://github.com/serde-rs/serde",
              "license": "MIT OR Apache-2.0",
              "categories": ["encoding"],
              "keywords": ["serde"]
            },
            {
              "name": "tokio",
              "version": "1.0.0",
              "description": "Runtime for async Rust",
              "downloads": 500000,
              "recent_downloads": 5000,
              "repository": "https://github.com/tokio-rs/tokio",
              "license": "MIT",
              "categories": ["async"],
              "keywords": ["async", "runtime"]
            }
          ]
        }"#;

        let receipt = convert_cargo_krate_json(json, "cargo-krate").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
        assert_eq!(receipt.verdict.counts.findings, 2);
    }

    #[test]
    fn test_convert_cargo_krate_json_empty() {
        let json = r#"{
          "crates": []
        }"#;

        let receipt = convert_cargo_krate_json(json, "cargo-krate").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }
}
