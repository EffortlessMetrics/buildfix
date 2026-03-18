use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct ClippyAdapter {
    sensor_id: String,
}

impl ClippyAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "clippy".to_string(),
        }
    }
}

impl Default for ClippyAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for ClippyAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_clippy_json(&content, &self.sensor_id)
    }
}

fn convert_clippy_json(content: &str, sensor_id: &str) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut error_count = 0u64;
    let mut warning_count = 0u64;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let message: ClippyMessage = match serde_json::from_str(trimmed) {
            Ok(msg) => msg,
            Err(_) => continue,
        };

        let Some(reason) = message.reason.as_deref() else {
            continue;
        };

        if reason != "compiler-message" {
            continue;
        }

        let Some(msg) = message.message else {
            continue;
        };

        let severity = map_severity(&msg.level);
        match severity {
            Severity::Error => error_count += 1,
            Severity::Warn => warning_count += 1,
            _ => {}
        }

        let check_id = msg.code.as_ref().map(|c| {
            if c.starts_with("clippy::") {
                c.replace("::", ".")
            } else {
                c.clone()
            }
        });

        let location = extract_location(&msg);

        findings.push(Finding {
            severity,
            check_id: check_id.clone(),
            code: msg.code,
            message: Some(msg.message),
            location,
            fingerprint: None,
            data: None,
        });
    }

    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else if warning_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("clippy.message.v1")
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
        Some("warning") | Some("warn") => Severity::Warn,
        _ => Severity::Warn,
    }
}

fn extract_location(msg: &ClippyMessageContent) -> Option<Location> {
    for span in &msg.spans {
        if span.file_name.is_empty() {
            continue;
        }

        return Some(Location {
            path: Utf8PathBuf::from(&span.file_name),
            line: span.line_start,
            column: span.column_start,
        });
    }

    None
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct ClippyMessage {
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    package_id: Option<String>,
    #[serde(default)]
    target: Option<ClippyTarget>,
    #[serde(default)]
    message: Option<ClippyMessageContent>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
#[serde(rename_all = "kebab-case")]
struct ClippyTarget {
    #[serde(default)]
    kind: Vec<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    src_path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ClippyMessageContent {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    message: String,
    #[serde(default)]
    spans: Vec<ClippySpan>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct ClippySpan {
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    line_start: Option<u64>,
    #[serde(default)]
    line_end: Option<u64>,
    #[serde(default)]
    column_start: Option<u64>,
    #[serde(default)]
    column_end: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = ClippyAdapter::new();
        assert_eq!(adapter.sensor_id(), "clippy");
    }

    #[test]
    fn test_convert_clippy_json_with_message() {
        let json = r#"{"reason": "compiler-message", "package_id": "my_crate 0.1.0 (path+file:///path/to/crate)", "target": {"kind": ["lib"], "name": "my_crate", "src_path": "/path/to/src/lib.rs"}, "message": {"code": "clippy::unused_imports", "level": "warning", "message": "unused import: `foo`", "spans": [{"file_name": "src/lib.rs", "line_start": 1, "line_end": 1, "column_start": 1, "column_end": 10}]}}
"#;

        let receipt = convert_clippy_json(json, "clippy").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(finding.check_id, Some("clippy.unused_imports".to_string()));
        assert_eq!(finding.message, Some("unused import: `foo`".to_string()));
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "src/lib.rs"
        );
        assert_eq!(finding.location.as_ref().unwrap().line, Some(1));
    }

    #[test]
    fn test_convert_clippy_json_maps_severity() {
        let json = r#"{"reason": "compiler-message", "message": {"code": "clippy::error", "level": "error", "message": "Error message", "spans": []}}
{"reason": "compiler-message", "message": {"code": "clippy::warning", "level": "warning", "message": "Warning message", "spans": []}}
{"reason": "compiler-message", "message": {"code": "clippy::warn", "level": "warn", "message": "Warn message", "spans": []}}
"#;

        let receipt = convert_clippy_json(json, "clippy").unwrap();

        assert_eq!(receipt.findings.len(), 3);
        assert_eq!(receipt.findings[0].severity, Severity::Error);
        assert_eq!(receipt.findings[1].severity, Severity::Warn);
        assert_eq!(receipt.findings[2].severity, Severity::Warn);
    }

    #[test]
    fn test_convert_clippy_json_calculates_counts() {
        let json = r#"{"reason": "compiler-message", "message": {"code": "clippy::E0001", "level": "error", "message": "Error 1", "spans": []}}
{"reason": "compiler-message", "message": {"code": "clippy::E0002", "level": "error", "message": "Error 2", "spans": []}}
{"reason": "compiler-message", "message": {"code": "clippy::W0001", "level": "warning", "message": "Warning", "spans": []}}
"#;

        let receipt = convert_clippy_json(json, "clippy").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.findings, 3);
        assert_eq!(receipt.verdict.counts.errors, 2);
        assert_eq!(receipt.verdict.counts.warnings, 1);
    }

    #[test]
    fn test_convert_clippy_json_empty_passes() {
        let json = r#""#;

        let receipt = convert_clippy_json(json, "clippy").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_clippy_json_skips_non_messages() {
        let json = r#"{"reason": "build-finished", "message": {"code": null, "level": "note", "message": "Build finished", "spans": []}}
{"reason": "compiler-message", "message": {"code": "clippy::warning", "level": "warning", "message": "Actual warning", "spans": []}}
"#;

        let receipt = convert_clippy_json(json, "clippy").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(
            receipt.findings[0].message,
            Some("Actual warning".to_string())
        );
    }

    #[test]
    fn test_convert_clippy_json_check_id_format() {
        let json = r#"{"reason": "compiler-message", "message": {"code": "clippy::double_comparison", "level": "warning", "message": "compare", "spans": [{"file_name": "src/main.rs", "line_start": 10, "line_end": 10, "column_start": 5, "column_end": 15}]}}
"#;

        let receipt = convert_clippy_json(json, "clippy").unwrap();

        assert_eq!(
            receipt.findings[0].check_id,
            Some("clippy.double_comparison".to_string())
        );
    }
}
