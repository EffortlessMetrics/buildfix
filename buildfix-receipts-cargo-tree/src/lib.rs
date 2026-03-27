use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoTreeAdapter {
    sensor_id: String,
}

impl CargoTreeAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-tree".to_string(),
        }
    }
}

impl Default for CargoTreeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoTreeAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_tree_json(&content, &self.sensor_id)
    }
}

impl AdapterMetadata for CargoTreeAdapter {
    fn name(&self) -> &str {
        "cargo-tree"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-tree.report.v1"]
    }
}

fn convert_cargo_tree_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let parsed: serde_json::Value = serde_json::from_str(content).map_err(AdapterError::Json)?;

    if parsed.get("duplicates").is_some() {
        convert_duplicates_json(&parsed, sensor_id)
    } else {
        convert_tree_json(&parsed, sensor_id)
    }
}

fn convert_tree_json(
    parsed: &serde_json::Value,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let report: CargoTreeReport =
        serde_json::from_value(parsed.clone()).map_err(AdapterError::Json)?;

    let pkg_count = report.packages.len();
    let total_deps = report
        .packages
        .iter()
        .map(|p| p.dependencies.len())
        .sum::<usize>();

    let builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-tree.report.v1")
        .with_tool_version(format!("{}", report.version))
        .with_status(VerdictStatus::Pass)
        .with_counts(pkg_count as u64, 0, total_deps as u64);

    let mut receipt = builder.build();
    receipt.data = Some(serde_json::json!({
        "format": report.format,
        "package_count": pkg_count,
        "total_dependencies": total_deps,
    }));

    Ok(receipt)
}

fn convert_duplicates_json(
    parsed: &serde_json::Value,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let duplicates: DuplicatesReport =
        serde_json::from_value(parsed.clone()).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();

    if let Some(dups) = duplicates.duplicates {
        for dup in dups {
            let versions_str = dup.versions.join(", ");

            let info_str: Vec<String> = dup
                .info
                .iter()
                .map(|i| format!("{} v{}", i.name, i.version))
                .collect();
            let info_str = info_str.join("; ");

            let message = format!(
                "Duplicate dependency '{}' has multiple versions: [{}]. Used by: {}",
                dup.dep, versions_str, info_str
            );

            let data = CargoTreeDuplicateData {
                dependency: dup.dep.clone(),
                versions: dup.versions.clone(),
                info: dup
                    .info
                    .iter()
                    .map(|i| DuplicateInfo {
                        name: i.name.clone(),
                        version: i.version.clone(),
                    })
                    .collect(),
            };

            findings.push(Finding {
                severity: Severity::Warn,
                check_id: Some("deps.duplicate_dependency_versions".to_string()),
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
    }

    let status = if !findings.is_empty() {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-tree.duplicates.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, findings.len() as u64);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CargoTreeReport {
    format: String,
    version: u32,
    packages: Vec<Package>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Package {
    name: String,
    version: String,
    manifest_path: String,
    dependencies: Vec<Dependency>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Dependency {
    name: String,
    version: String,
    dependencies: Option<Vec<Dependency>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DuplicatesReport {
    duplicates: Option<Vec<Duplicate>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Duplicate {
    dep: String,
    versions: Vec<String>,
    info: Vec<DuplicateInfoRaw>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DuplicateInfoRaw {
    name: String,
    version: String,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct CargoTreeDuplicateData {
    dependency: String,
    versions: Vec<String>,
    info: Vec<DuplicateInfo>,
}

#[derive(Debug, serde::Serialize)]
#[allow(dead_code)]
struct DuplicateInfo {
    name: String,
    version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoTreeAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-tree");
    }

    #[test]
    fn test_convert_cargo_tree_json_normal() {
        let json = r#"{
          "format": "json",
          "version": 1,
          "packages": [
            {
              "name": "my-crate",
              "version": "0.1.0",
              "manifest_path": "/path/to/Cargo.toml",
              "dependencies": [
                {
                  "name": "dep-a",
                  "version": "1.0.0",
                  "dependencies": []
                }
              ]
            }
          ]
        }"#;

        let receipt = convert_cargo_tree_json(json, "cargo-tree").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_duplicates_json_with_duplicates() {
        let json = r#"{
          "duplicates": [
            {
              "dep": "serde",
              "versions": ["1.0.0", "1.0.1"],
              "info": [
                {"name": "package-a", "version": "1.0.0"},
                {"name": "package-b", "version": "1.0.1"}
              ]
            }
          ]
        }"#;

        let receipt = convert_cargo_tree_json(json, "cargo-tree").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(
            finding.check_id,
            Some("deps.duplicate_dependency_versions".to_string())
        );
        assert!(finding.message.as_ref().unwrap().contains("serde"));
        assert!(finding.message.as_ref().unwrap().contains("1.0.0"));
        assert!(finding.message.as_ref().unwrap().contains("1.0.1"));
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "Cargo.toml"
        );
    }

    #[test]
    fn test_convert_duplicates_json_multiple() {
        let json = r#"{
          "duplicates": [
            {
              "dep": "serde",
              "versions": ["1.0.0", "1.0.1"],
              "info": [
                {"name": "package-a", "version": "1.0.0"},
                {"name": "package-b", "version": "1.0.1"}
              ]
            },
            {
              "dep": "tokio",
              "versions": ["1.0.0", "1.1.0"],
              "info": [
                {"name": "crate-x", "version": "1.0.0"},
                {"name": "crate-y", "version": "1.1.0"}
              ]
            }
          ]
        }"#;

        let receipt = convert_cargo_tree_json(json, "cargo-tree").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_duplicates_json_empty() {
        let json = r#"{
          "duplicates": []
        }"#;

        let receipt = convert_cargo_tree_json(json, "cargo-tree").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_duplicates_json_no_duplicates() {
        let json = r#"{
          "duplicates": null
        }"#;

        let receipt = convert_cargo_tree_json(json, "cargo-tree").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }
}
