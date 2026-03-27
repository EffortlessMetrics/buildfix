use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoGeigerAdapter {
    sensor_id: String,
}

impl CargoGeigerAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-geiger".to_string(),
        }
    }
}

impl Default for CargoGeigerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoGeigerAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_geiger_json(&content, &self.sensor_id)
    }
}

impl AdapterMetadata for CargoGeigerAdapter {
    fn name(&self) -> &str {
        "cargo-geiger"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-geiger.report.v1"]
    }
}

fn convert_cargo_geiger_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let report: CargoGeigerReport = serde_json::from_str(content).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();
    let mut warning_count = 0u64;

    if let Some(files) = &report.files {
        for file in files {
            let total_unsafe = file.functions.values().sum::<u64>()
                + file.lines.get("unsafe").copied().unwrap_or(0);

            if total_unsafe > 0 {
                warning_count += 1;

                let message = format!(
                    "Unsafe usage: {} unsafe function(s), {} unsafe block(s), {} unsafe line(s)",
                    file.functions.values().sum::<u64>(),
                    file.functions.get("unsafe block").copied().unwrap_or(0),
                    file.lines.get("unsafe").copied().unwrap_or(0)
                );

                let location = Location {
                    path: Utf8PathBuf::from(&file.path),
                    line: None,
                    column: None,
                };

                let percentage = file.percentage.unwrap_or(0.0);

                let check_id = if percentage > 50.0 {
                    "geiger.unsafe_count".to_string()
                } else {
                    "safety.unsafe_usage".to_string()
                };

                let data = serde_json::json!({
                    "functions": file.functions,
                    "lines": file.lines,
                    "percentage": percentage
                });

                findings.push(Finding {
                    severity: Severity::Warn,
                    check_id: Some(check_id),
                    code: None,
                    message: Some(message),
                    location: Some(location),
                    fingerprint: None,
                    data: Some(data),
                    ..Default::default()
                });
            }
        }
    }

    if let Some(dependencies) = &report.dependencies {
        for dep in dependencies {
            let total_unsafe = dep.unsafe_functions + dep.unsafe_traits + dep.unsafe_blocks;

            if total_unsafe > 0 {
                warning_count += 1;

                let message = format!(
                    "Dependency '{}' has unsafe usage: {} unsafe function(s), {} unsafe trait(s), {} unsafe block(s)",
                    dep.name, dep.unsafe_functions, dep.unsafe_traits, dep.unsafe_blocks
                );

                let data = serde_json::json!({
                    "dependency_name": dep.name,
                    "unsafe_functions": dep.unsafe_functions,
                    "unsafe_traits": dep.unsafe_traits,
                    "unsafe_blocks": dep.unsafe_blocks
                });

                findings.push(Finding {
                    severity: Severity::Warn,
                    check_id: Some("safety.unsafe_usage".to_string()),
                    code: None,
                    message: Some(message),
                    location: None,
                    fingerprint: None,
                    data: Some(data),
                    ..Default::default()
                });
            }
        }
    }

    let status = if warning_count > 0 || !findings.is_empty() {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-geiger.report.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, warning_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CargoGeigerReport {
    files: Option<Vec<FileEntry>>,
    dependencies: Option<Vec<DependencyEntry>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FileEntry {
    path: String,
    functions: std::collections::HashMap<String, u64>,
    lines: std::collections::HashMap<String, u64>,
    percentage: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DependencyEntry {
    name: String,
    #[serde(rename = "unsafe_functions")]
    unsafe_functions: u64,
    #[serde(rename = "unsafe_traits")]
    unsafe_traits: u64,
    #[serde(rename = "unsafe_blocks")]
    unsafe_blocks: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoGeigerAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-geiger");
    }

    #[test]
    fn test_convert_cargo_geiger_json_with_files() {
        let json = r#"{
  "files": [
    {
      "path": "src/lib.rs",
      "functions": {
        "unsafe_fn": 1,
        "unsafe_call": 2,
        "unsafe block": 3
      },
      "lines": {
        "unsafe": 10,
        "total": 100
      },
      "percentage": 10.0
    }
  ],
  "dependencies": []
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(finding.check_id, Some("safety.unsafe_usage".to_string()));
        assert!(finding.message.as_ref().unwrap().contains("Unsafe usage"));
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "src/lib.rs"
        );
    }

    #[test]
    fn test_convert_cargo_geiger_json_high_percentage() {
        let json = r#"{
  "files": [
    {
      "path": "src/lib.rs",
      "functions": {
        "unsafe_fn": 5,
        "unsafe block": 10
      },
      "lines": {
        "unsafe": 60,
        "total": 100
      },
      "percentage": 60.0
    }
  ],
  "dependencies": []
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.check_id, Some("geiger.unsafe_count".to_string()));
    }

    #[test]
    fn test_convert_cargo_geiger_json_with_dependencies() {
        let json = r#"{
  "files": [],
  "dependencies": [
    {
      "name": "some-crate",
      "unsafe_functions": 5,
      "unsafe_traits": 1,
      "unsafe_blocks": 2
    }
  ]
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(finding.check_id, Some("safety.unsafe_usage".to_string()));
        assert!(finding.message.as_ref().unwrap().contains("some-crate"));
    }

    #[test]
    fn test_convert_cargo_geiger_json_empty_passes() {
        let json = r#"{
  "files": [],
  "dependencies": []
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_geiger_json_no_unsafe_passes() {
        let json = r#"{
  "files": [
    {
      "path": "src/lib.rs",
      "functions": {},
      "lines": {
        "unsafe": 0,
        "total": 100
      },
      "percentage": 0.0
    }
  ],
  "dependencies": [
    {
      "name": "safe-crate",
      "unsafe_functions": 0,
      "unsafe_traits": 0,
      "unsafe_blocks": 0
    }
  ]
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_geiger_json_calculates_counts() {
        let json = r#"{
  "files": [
    {
      "path": "src/lib.rs",
      "functions": {
        "unsafe_fn": 1
      },
      "lines": {
        "unsafe": 5
      },
      "percentage": 5.0
    },
    {
      "path": "src/main.rs",
      "functions": {
        "unsafe block": 2
      },
      "lines": {
        "unsafe": 3
      },
      "percentage": 3.0
    }
  ],
  "dependencies": [
    {
      "name": "unsafe-dep",
      "unsafe_functions": 1,
      "unsafe_traits": 0,
      "unsafe_blocks": 0
    }
  ]
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.findings.len(), 3);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 3);
        assert_eq!(receipt.verdict.counts.warnings, 3);
    }

    #[test]
    fn test_adapter_loads_from_file() {
        let adapter = CargoGeigerAdapter::new();
        let receipt = adapter
            .load(Path::new("tests/fixtures/report.json"))
            .expect("should load fixture");

        assert_eq!(adapter.sensor_id(), "cargo-geiger");
        assert!(!receipt.findings.is_empty());
    }

    #[test]
    fn test_adapter_returns_error_for_missing_file() {
        let adapter = CargoGeigerAdapter::new();
        let result = adapter.load(Path::new("nonexistent/path.json"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Io(_)));
    }

    #[test]
    fn test_adapter_returns_error_for_invalid_json() {
        let adapter = CargoGeigerAdapter::new();

        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().join("invalid.json");
        std::fs::write(&temp_path, "{ invalid json }").unwrap();

        let result = adapter.load(&temp_path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Json(_)));
    }

    #[test]
    fn test_convert_cargo_geiger_json_null_values() {
        let json = r#"{
  "files": [
    {
      "path": "src/lib.rs",
      "functions": {
        "unsafe_fn": 1
      },
      "lines": {
        "unsafe": 5
      },
      "percentage": null
    }
  ],
  "dependencies": null
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }

    #[test]
    fn test_convert_cargo_geiger_json_missing_fields() {
        let json = r#"{
  "files": [
    {
      "path": "src/lib.rs",
      "functions": {},
      "lines": {},
      "percentage": null
    }
  ]
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_geiger_json_empty_dependencies_array() {
        let json = r#"{
  "files": [],
  "dependencies": []
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_severity_mapping() {
        let json = r#"{
  "files": [
    {
      "path": "src/lib.rs",
      "functions": {
        "unsafe_fn": 1
      },
      "lines": {
        "unsafe": 1
      },
      "percentage": 1.0
    }
  ],
  "dependencies": []
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
    }

    #[test]
    fn test_verdict_with_only_warnings() {
        let json = r#"{
  "files": [
    {
      "path": "src/lib.rs",
      "functions": {
        "unsafe_fn": 1
      },
      "lines": {
        "unsafe": 5
      },
      "percentage": 5.0
    }
  ],
  "dependencies": []
}"#;

        let receipt = convert_cargo_geiger_json(json, "cargo-geiger").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert!(receipt.verdict.counts.warnings > 0);
    }
}
