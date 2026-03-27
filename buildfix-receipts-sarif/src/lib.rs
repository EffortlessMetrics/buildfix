use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct SarifAdapter {
    sensor_id: String,
}

impl SarifAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "sarif".to_string(),
        }
    }

    pub fn with_tool_name(mut self, name: impl Into<String>) -> Self {
        self.sensor_id = format!("sarif-{}", name.into().to_lowercase());
        self
    }
}

impl Default for SarifAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for SarifAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: SarifLog = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_sarif(report, &self.sensor_id)
    }
}

impl AdapterMetadata for SarifAdapter {
    fn name(&self) -> &str {
        "sarif"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["sarif.report.v1"]
    }
}

fn convert_sarif(sarif: SarifLog, sensor_id: &str) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut error_count = 0u64;
    let mut warning_count = 0u64;

    let mut tool_name = "unknown".to_string();
    let mut tool_version = None;

    for run in &sarif.runs {
        if let Some(tool) = &run.tool
            && let Some(driver) = &tool.driver
        {
            tool_name = driver.name.clone();
            tool_version = driver.version.clone();
        }

        if let Some(results) = &run.results {
            for result in results {
                let severity = map_severity(&result.level);
                match severity {
                    Severity::Error => error_count += 1,
                    Severity::Warn => warning_count += 1,
                    _ => {}
                }

                let message = result
                    .message
                    .as_ref()
                    .and_then(|m| m.text.clone())
                    .unwrap_or_default();

                let location = extract_location(result);

                findings.push(Finding {
                    severity,
                    check_id: result.rule_id.clone(),
                    code: result.rule_id.clone(),
                    message: if message.is_empty() {
                        None
                    } else {
                        Some(message)
                    },
                    location,
                    fingerprint: None,
                    data: None,
                    ..Default::default()
                });
            }
        }
    }

    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else if warning_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let actual_sensor_id = if sensor_id == "sarif" {
        format!("sarif-{}", tool_name.to_lowercase())
    } else {
        sensor_id.to_string()
    };

    let mut builder = ReceiptBuilder::new(actual_sensor_id)
        .with_schema("sarif.report.v1")
        .with_tool_version(tool_version.unwrap_or_else(|| "0.0.0".to_string()))
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, warning_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

fn map_severity(level: &Option<String>) -> Severity {
    match level.as_deref() {
        Some("error") => Severity::Error,
        Some("warning") => Severity::Warn,
        Some("note") => Severity::Info,
        Some("none") => Severity::Info,
        _ => Severity::Info,
    }
}

fn extract_location(result: &SarifResult) -> Option<Location> {
    let locations = result.locations.as_ref()?;

    for loc in locations {
        if let Some(physical) = &loc.physical_location
            && let Some(artifact) = &physical.artifact_location
        {
            let path = artifact.uri.clone().unwrap_or_default();
            let line = physical.region.as_ref().and_then(|r| r.start_line);

            if !path.is_empty() || line.is_some() {
                return Some(Location {
                    path: Utf8PathBuf::from(path),
                    line,
                    column: physical.region.as_ref().and_then(|r| r.start_column),
                });
            }
        }
    }

    None
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifLog {
    #[serde(default)]
    runs: Vec<SarifRun>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifRun {
    #[serde(default)]
    tool: Option<SarifTool>,
    #[serde(default)]
    results: Option<Vec<SarifResult>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifTool {
    #[serde(default)]
    driver: Option<SarifDriver>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifDriver {
    name: String,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    #[serde(default)]
    rule_id: Option<String>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    message: Option<SarifMessage>,
    #[serde(default)]
    locations: Option<Vec<SarifLocation>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifMessage {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    #[serde(default)]
    physical_location: Option<SarifPhysicalLocation>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    #[serde(default)]
    artifact_location: Option<SarifArtifactLocation>,
    #[serde(default)]
    region: Option<SarifRegion>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifArtifactLocation {
    #[serde(default)]
    uri: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    #[serde(default)]
    start_line: Option<u64>,
    #[serde(default)]
    start_column: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = SarifAdapter::new();
        assert_eq!(adapter.sensor_id(), "sarif");
    }

    #[test]
    fn test_adapter_with_tool_name() {
        let adapter = SarifAdapter::new().with_tool_name("clippy");
        assert_eq!(adapter.sensor_id(), "sarif-clippy");
    }

    #[test]
    fn test_adapter_loads_from_file_fixture() {
        let json = r#"{
            "runs": [
                {
                    "tool": {
                        "driver": {
                            "name": "CodeScanner",
                            "version": "2.1.0"
                        }
                    },
                    "results": [
                        {
                            "ruleId": "SEC001",
                            "level": "error",
                            "message": {
                                "text": "Hardcoded credential found"
                            },
                            "locations": [
                                {
                                    "physicalLocation": {
                                        "artifactLocation": {
                                            "uri": "config.yaml"
                                        },
                                        "region": {
                                            "startLine": 10,
                                            "startColumn": 15
                                        }
                                    }
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        use std::io::Write;
        temp_file.write_all(json.as_bytes()).unwrap();

        let adapter = SarifAdapter::new();
        let result = adapter.load(temp_file.path());

        assert!(result.is_ok());
        let receipt = result.unwrap();
        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].check_id, Some("SEC001".to_string()));
    }

    #[test]
    fn test_adapter_returns_error_for_missing_file() {
        let adapter = SarifAdapter::new();
        let result = adapter.load(Path::new("/nonexistent/path/sarif.json"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Io(_)));
    }

    #[test]
    fn test_adapter_returns_error_for_invalid_json() {
        let json = "this is not valid json {{{";

        let mut temp_file = NamedTempFile::new().unwrap();
        use std::io::Write;
        temp_file.write_all(json.as_bytes()).unwrap();

        let adapter = SarifAdapter::new();
        let result = adapter.load(temp_file.path());

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Json(_)));
    }

    #[test]
    fn test_severity_mapping_error() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "ERR001",
                    "level": "error",
                    "message": { "text": "Error message" }
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Error);
    }

    #[test]
    fn test_severity_mapping_warning() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "WARN001",
                    "level": "warning",
                    "message": { "text": "Warning message" }
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Warn);
    }

    #[test]
    fn test_severity_mapping_note() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "NOTE001",
                    "level": "note",
                    "message": { "text": "Note message" }
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Info);
    }

    #[test]
    fn test_severity_mapping_unknown_defaults_to_info() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "UNKNOWN001",
                    "level": "invalid",
                    "message": { "text": "Unknown level" }
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Info);
    }

    #[test]
    fn test_severity_mapping_none_to_info() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "NONE001",
                    "level": "none",
                    "message": { "text": "None level" }
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Info);
    }

    #[test]
    fn test_severity_mapping_missing_level_defaults_to_info() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "NOLEVEL001",
                    "message": { "text": "No level specified" }
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Info);
    }

    #[test]
    fn test_verdict_fail_with_errors() {
        let json = r#"{
            "runs": [{
                "results": [
                    { "ruleId": "E1", "level": "error", "message": { "text": "e1" } },
                    { "ruleId": "E2", "level": "error", "message": { "text": "e2" } }
                ]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.errors, 2);
    }

    #[test]
    fn test_verdict_warn_without_errors() {
        let json = r#"{
            "runs": [{
                "results": [
                    { "ruleId": "W1", "level": "warning", "message": { "text": "w1" } },
                    { "ruleId": "W2", "level": "warning", "message": { "text": "w2" } },
                    { "ruleId": "N1", "level": "note", "message": { "text": "n1" } }
                ]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.errors, 0);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_verdict_pass_with_only_notes() {
        let json = r#"{
            "runs": [{
                "results": [
                    { "ruleId": "N1", "level": "note", "message": { "text": "n1" } },
                    { "ruleId": "N2", "level": "note", "message": { "text": "n2" } }
                ]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_location_with_line_and_column() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "message": { "text": "m" },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": { "uri": "src/main.rs" },
                            "region": { "startLine": 25, "startColumn": 10 }
                        }
                    }]
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        let location = receipt.findings[0].location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "src/main.rs");
        assert_eq!(location.line, Some(25));
        assert_eq!(location.column, Some(10));
    }

    #[test]
    fn test_location_with_only_line() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "message": { "text": "m" },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": { "uri": "src/lib.rs" },
                            "region": { "startLine": 100 }
                        }
                    }]
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        let location = receipt.findings[0].location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "src/lib.rs");
        assert_eq!(location.line, Some(100));
        assert_eq!(location.column, None);
    }

    #[test]
    fn test_location_with_only_path() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "message": { "text": "m" },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": { "uri": "Cargo.toml" }
                        }
                    }]
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        let location = receipt.findings[0].location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "Cargo.toml");
        assert_eq!(location.line, None);
    }

    #[test]
    fn test_location_without_region() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "message": { "text": "m" },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": { "uri": "README.md" },
                            "region": {}
                        }
                    }]
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        let location = receipt.findings[0].location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "README.md");
        assert_eq!(location.line, None);
    }

    #[test]
    fn test_result_without_location() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "level": "error",
                    "message": { "text": "Error without location" }
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert!(receipt.findings[0].location.is_none());
    }

    #[test]
    fn test_result_with_empty_locations_array() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "message": { "text": "m" },
                    "locations": []
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert!(receipt.findings[0].location.is_none());
    }

    #[test]
    fn test_multiple_runs_in_sarif() {
        let json = r#"{
            "runs": [
                {
                    "tool": { "driver": { "name": "ToolA", "version": "1.0" } },
                    "results": [
                        { "ruleId": "A1", "level": "error", "message": { "text": "a1" } }
                    ]
                },
                {
                    "tool": { "driver": { "name": "ToolB", "version": "2.0" } },
                    "results": [
                        { "ruleId": "B1", "level": "warning", "message": { "text": "b1" } },
                        { "ruleId": "B2", "level": "error", "message": { "text": "b2" } }
                    ]
                }
            ]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 3);
        assert_eq!(receipt.verdict.counts.errors, 2);
        assert_eq!(receipt.verdict.counts.warnings, 1);
        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
    }

    #[test]
    fn test_multiple_runs_uses_last_tool_info() {
        let json = r#"{
            "runs": [
                {
                    "tool": { "driver": { "name": "FirstTool", "version": "1.0.0" } },
                    "results": []
                },
                {
                    "tool": { "driver": { "name": "SecondTool", "version": "2.0.0" } },
                    "results": []
                }
            ]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.tool.name, "sarif-secondtool");
    }

    #[test]
    fn test_tool_information_extraction() {
        let json = r#"{
            "runs": [{
                "tool": {
                    "driver": {
                        "name": "Semgrep",
                        "version": "1.45.0"
                    }
                },
                "results": []
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.tool.name, "sarif-semgrep");
    }

    #[test]
    fn test_tool_information_missing_version() {
        let json = r#"{
            "runs": [{
                "tool": {
                    "driver": {
                        "name": "Scanner"
                    }
                },
                "results": []
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.tool.name, "sarif-scanner");
    }

    #[test]
    fn test_tool_information_missing_driver() {
        let json = r#"{
            "runs": [{
                "tool": {},
                "results": []
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.tool.name, "sarif-unknown");
    }

    #[test]
    fn test_message_extraction_from_text() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "message": { "text": "Explicit message text" }
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(
            receipt.findings[0].message,
            Some("Explicit message text".to_string())
        );
    }

    #[test]
    fn test_message_extraction_missing_message_field() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1"
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings[0].message, None);
    }

    #[test]
    fn test_message_extraction_empty_text() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "message": {}
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings[0].message, None);
    }

    #[test]
    fn test_artifact_location_with_uri_base_id() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "message": { "text": "m" },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": {
                                "uri": "file:///workspace/src/main.rs",
                                "uriBaseId": "SRCROOT"
                            },
                            "region": { "startLine": 5 }
                        }
                    }]
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        let location = receipt.findings[0].location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "file:///workspace/src/main.rs");
    }

    #[test]
    fn test_artifact_location_empty_uri() {
        let json = r#"{
            "runs": [{
                "results": [{
                    "ruleId": "R1",
                    "message": { "text": "m" },
                    "locations": [{
                        "physicalLocation": {
                            "artifactLocation": {},
                            "region": { "startLine": 1 }
                        }
                    }]
                }]
            }]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        let location = receipt.findings[0].location.as_ref().unwrap();
        assert!(location.path.as_str().is_empty());
    }

    #[test]
    fn test_convert_sarif_with_results() {
        let json = r#"{
            "runs": [
                {
                    "tool": {
                        "driver": {
                            "name": "CodeScanner",
                            "version": "1.0.0"
                        }
                    },
                    "results": [
                        {
                            "ruleId": "CWE-79",
                            "level": "error",
                            "message": {
                                "text": "Cross-site scripting (XSS) vulnerability"
                            },
                            "locations": [
                                {
                                    "physicalLocation": {
                                        "artifactLocation": {
                                            "uri": "src/index.js"
                                        },
                                        "region": {
                                            "startLine": 42,
                                            "startColumn": 5
                                        }
                                    }
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Error);
        assert_eq!(finding.check_id, Some("CWE-79".to_string()));
        assert_eq!(
            finding.message,
            Some("Cross-site scripting (XSS) vulnerability".to_string())
        );
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "src/index.js"
        );
        assert_eq!(finding.location.as_ref().unwrap().line, Some(42));
    }

    #[test]
    fn test_convert_sarif_maps_severity() {
        let json = r#"{
            "runs": [
                {
                    "tool": {
                        "driver": {
                            "name": "Scanner",
                            "version": "1.0"
                        }
                    },
                    "results": [
                        {
                            "ruleId": "ERR001",
                            "level": "error",
                            "message": { "text": "Error" }
                        },
                        {
                            "ruleId": "WARN001",
                            "level": "warning",
                            "message": { "text": "Warning" }
                        },
                        {
                            "ruleId": "INFO001",
                            "level": "note",
                            "message": { "text": "Info" }
                        }
                    ]
                }
            ]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 3);
        assert_eq!(receipt.findings[0].severity, Severity::Error);
        assert_eq!(receipt.findings[1].severity, Severity::Warn);
        assert_eq!(receipt.findings[2].severity, Severity::Info);
    }

    #[test]
    fn test_convert_sarif_calculates_counts() {
        let json = r#"{
            "runs": [
                {
                    "tool": {
                        "driver": {
                            "name": "Scanner",
                            "version": "1.0"
                        }
                    },
                    "results": [
                        {
                            "ruleId": "ERR001",
                            "level": "error",
                            "message": { "text": "Error 1" }
                        },
                        {
                            "ruleId": "ERR002",
                            "level": "error",
                            "message": { "text": "Error 2" }
                        },
                        {
                            "ruleId": "WARN001",
                            "level": "warning",
                            "message": { "text": "Warning" }
                        }
                    ]
                }
            ]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.findings, 3);
        assert_eq!(receipt.verdict.counts.errors, 2);
        assert_eq!(receipt.verdict.counts.warnings, 1);
    }

    #[test]
    fn test_convert_sarif_empty_passes() {
        let json = r#"{
            "runs": [
                {
                    "tool": {
                        "driver": {
                            "name": "Scanner",
                            "version": "1.0"
                        }
                    },
                    "results": []
                }
            ]
        }"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_sarif_no_runs_passes() {
        let json = r#"{}"#;

        let sarif: SarifLog = serde_json::from_str(json).unwrap();
        let receipt = convert_sarif(sarif, "sarif").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }
}
