use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct DepguardAdapter;

impl DepguardAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DepguardAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for DepguardAdapter {
    fn sensor_id(&self) -> &str {
        "depguard"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;

        let parsed: serde_json::Value =
            serde_json::from_str(&content).map_err(AdapterError::Json)?;

        if let Some(arr) = parsed.as_array()
            && !arr.is_empty()
            && arr[0].get("manifest_path").is_some()
        {
            let report: DepguardArrayReport =
                serde_json::from_value(parsed).map_err(AdapterError::Json)?;
            return convert_array_report(report);
        }

        if let Some(obj) = parsed.as_object()
            && obj.contains_key("files")
        {
            let report: DepguardFilesReport =
                serde_json::from_value(parsed).map_err(AdapterError::Json)?;
            return convert_files_report(report);
        }

        Err(AdapterError::InvalidFormat(
            "Unknown depguard output format".to_string(),
        ))
    }
}

impl AdapterMetadata for DepguardAdapter {
    fn name(&self) -> &str {
        "depguard"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["depguard.report.v1"]
    }
}

fn convert_array_report(report: DepguardArrayReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut warn_count = 0u64;

    for item in &report.0 {
        if let Some(violations) = &item.violations {
            for violation in violations {
                let check_id = map_check_id(&violation.violation_type);

                let location = Location {
                    path: Utf8PathBuf::from(&item.manifest_path),
                    line: None,
                    column: None,
                };

                let message = format!(
                    "depguard: {} - {}",
                    violation.violation_type.replace(['_', '-'], " "),
                    violation.dependency.as_deref().unwrap_or("unknown")
                );

                let data = DepguardViolationData {
                    dependency: violation.dependency.clone(),
                    violation_type: violation.violation_type.clone(),
                };

                findings.push(Finding {
                    severity: Severity::Warn,
                    check_id: Some(check_id),
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
    }

    let status = if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new("depguard")
        .with_schema("depguard.report.v1")
        .with_tool_version("0.0.0")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, warn_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    let receipt = builder.build();

    Ok(receipt)
}

fn convert_files_report(report: DepguardFilesReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut warn_count = 0u64;

    if let Some(files) = &report.files {
        for file in files {
            if let Some(messages) = &file.messages {
                for msg in messages {
                    let check_id = map_check_id(&msg.violation_type);

                    let location = Location {
                        path: Utf8PathBuf::from(&file.path),
                        line: msg.line,
                        column: msg.column,
                    };

                    let message = msg.message.clone();

                    let data = DepguardMessageData {
                        code: msg.code.clone(),
                        violation_type: msg.violation_type.clone(),
                    };

                    findings.push(Finding {
                        severity: Severity::Warn,
                        check_id: Some(check_id),
                        code: msg.code.clone(),
                        message: Some(message),
                        location: Some(location),
                        fingerprint: None,
                        data: Some(serde_json::to_value(data).unwrap_or_default()),
                        ..Default::default()
                    });

                    warn_count += 1;
                }
            }
        }
    }

    let status = if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new("depguard")
        .with_schema("depguard.report.v1")
        .with_tool_version("0.0.0")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, warn_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    let receipt = builder.build();

    Ok(receipt)
}

fn map_check_id(violation_type: &str) -> String {
    match violation_type {
        "path_requires_version" => "deps.path_requires_version".to_string(),
        "workspace_inheritance" => "deps.workspace_inheritance".to_string(),
        "duplicate_dependency_versions" | "duplicate_versions" => {
            "deps.duplicate_dependency_versions".to_string()
        }
        _ => format!("deps.{}", violation_type),
    }
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct DepguardArrayReport(Vec<DepguardArrayItem>);

#[derive(Debug, Deserialize)]
struct DepguardArrayItem {
    manifest_path: String,
    #[serde(default)]
    violations: Option<Vec<DepguardViolation>>,
}

#[derive(Debug, Deserialize, Clone)]
struct DepguardViolation {
    dependency: Option<String>,
    #[serde(rename = "type")]
    violation_type: String,
}

#[derive(Debug, Deserialize)]
struct DepguardFilesReport {
    #[serde(default)]
    files: Option<Vec<DepguardFile>>,
}

#[derive(Debug, Deserialize)]
struct DepguardFile {
    path: String,
    #[serde(default)]
    messages: Option<Vec<DepguardMessage>>,
}

#[derive(Debug, Deserialize)]
struct DepguardMessage {
    message: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(rename = "type")]
    violation_type: String,
    #[serde(default)]
    line: Option<u64>,
    #[serde(default)]
    column: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DepguardViolationData {
    dependency: Option<String>,
    violation_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DepguardMessageData {
    code: Option<String>,
    violation_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = DepguardAdapter::new();
        assert_eq!(adapter.sensor_id(), "depguard");
    }

    #[test]
    fn test_convert_array_format_with_violations() {
        let json = r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "foo",
                        "type": "path_requires_version"
                    }
                ]
            }
        ]"#;

        let report: DepguardArrayReport = serde_json::from_str(json).unwrap();
        let receipt = convert_array_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(
            finding.check_id,
            Some("deps.path_requires_version".to_string())
        );
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }

    #[test]
    fn test_convert_files_format_with_messages() {
        let json = r#"{
            "files": [
                {
                    "path": "/path/to/Cargo.toml",
                    "messages": [
                        {
                            "message": "path dependency foo should have a version",
                            "code": "E001",
                            "type": "path_requires_version"
                        }
                    ]
                }
            ]
        }"#;

        let report: DepguardFilesReport = serde_json::from_str(json).unwrap();
        let receipt = convert_files_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(
            finding.check_id,
            Some("deps.path_requires_version".to_string())
        );
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "/path/to/Cargo.toml"
        );
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }

    #[test]
    fn test_convert_workspace_inheritance() {
        let json = r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "bar",
                        "type": "workspace_inheritance"
                    }
                ]
            }
        ]"#;

        let report: DepguardArrayReport = serde_json::from_str(json).unwrap();
        let receipt = convert_array_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(
            finding.check_id,
            Some("deps.workspace_inheritance".to_string())
        );
    }

    #[test]
    fn test_convert_duplicate_versions() {
        let json = r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "baz",
                        "type": "duplicate_dependency_versions"
                    }
                ]
            }
        ]"#;

        let report: DepguardArrayReport = serde_json::from_str(json).unwrap();
        let receipt = convert_array_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(
            finding.check_id,
            Some("deps.duplicate_dependency_versions".to_string())
        );
    }

    #[test]
    fn test_convert_duplicate_versions_short() {
        let json = r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {
                        "dependency": "baz",
                        "type": "duplicate_versions"
                    }
                ]
            }
        ]"#;

        let report: DepguardArrayReport = serde_json::from_str(json).unwrap();
        let receipt = convert_array_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(
            finding.check_id,
            Some("deps.duplicate_dependency_versions".to_string())
        );
    }

    #[test]
    fn test_convert_empty_array_passes() {
        let json = r#"[]"#;

        let report: DepguardArrayReport = serde_json::from_str(json).unwrap();
        let receipt = convert_array_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_empty_files_passes() {
        let json = r#"{"files": []}"#;

        let report: DepguardFilesReport = serde_json::from_str(json).unwrap();
        let receipt = convert_files_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_files_with_no_messages_passes() {
        let json = r#"{"files": [{"path": "/path/to/Cargo.toml"}]}"#;

        let report: DepguardFilesReport = serde_json::from_str(json).unwrap();
        let receipt = convert_files_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_multiple_findings() {
        let json = r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {"dependency": "foo", "type": "path_requires_version"},
                    {"dependency": "bar", "type": "workspace_inheritance"},
                    {"dependency": "baz", "type": "duplicate_dependency_versions"}
                ]
            }
        ]"#;

        let report: DepguardArrayReport = serde_json::from_str(json).unwrap();
        let receipt = convert_array_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 3);
        assert_eq!(receipt.verdict.counts.findings, 3);
        assert_eq!(receipt.verdict.counts.warnings, 3);
    }

    #[test]
    fn test_unknown_violation_type() {
        let json = r#"[
            {
                "manifest_path": "/path/to/Cargo.toml",
                "violations": [
                    {"dependency": "foo", "type": "unknown_check"}
                ]
            }
        ]"#;

        let report: DepguardArrayReport = serde_json::from_str(json).unwrap();
        let receipt = convert_array_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.check_id, Some("deps.unknown_check".to_string()));
    }
}
