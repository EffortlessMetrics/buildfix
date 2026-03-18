use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct MiriAdapter {
    sensor_id: String,
}

impl MiriAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-miri".to_string(),
        }
    }
}

impl Default for MiriAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for MiriAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_miri_json(&content, &self.sensor_id)
    }
}

fn convert_miri_json(content: &str, sensor_id: &str) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut error_count = 0u64;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let message: MiriMessage = match serde_json::from_str(trimmed) {
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

        error_count += 1;

        let check_id = if let Some(ref code) = msg.code {
            match code.code.as_str() {
                "undef" => Some("miri.undefined_behavior".to_string()),
                _ => Some("miri.error".to_string()),
            }
        } else {
            Some("miri.error".to_string())
        };

        let location = extract_location(&msg);

        findings.push(Finding {
            severity: Severity::Error,
            check_id,
            code: msg.code.map(|c| c.code),
            message: Some(msg.message),
            location,
            fingerprint: None,
            data: None,
        });
    }

    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("miri.message.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, 0);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

fn extract_location(msg: &MiriMessageContent) -> Option<Location> {
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
struct MiriMessage {
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    package_id: Option<String>,
    #[serde(default)]
    target: Option<MiriTarget>,
    #[serde(default)]
    message: Option<MiriMessageContent>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
#[serde(rename_all = "kebab-case")]
struct MiriTarget {
    #[serde(default)]
    kind: Vec<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    src_path: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct MiriMessageContent {
    #[serde(default)]
    code: Option<MiriCode>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    message: String,
    #[serde(default)]
    spans: Vec<MiriSpan>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct MiriCode {
    #[serde(default)]
    code: String,
    #[serde(default)]
    explanation: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct MiriSpan {
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
        let adapter = MiriAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-miri");
    }

    #[test]
    fn test_convert_miri_json_with_undef() {
        let json = r#"{"reason": "compiler-message", "package_id": "mycrate 0.1.0 (path+file:///path/to/crate)", "target": {"kind": ["lib"], "name": "mycrate", "src_path": "/path/to/src/lib.rs"}, "message": {"code": {"code": "undef", "explanation": "Undefined Behavior"}, "level": "error", "message": "using uninitialized data", "spans": [{"file_name": "src/lib.rs", "line_start": 10, "line_end": 10, "column_start": 5, "column_end": 8}]}}
"#;

        let receipt = convert_miri_json(json, "cargo-miri").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Error);
        assert_eq!(
            finding.check_id,
            Some("miri.undefined_behavior".to_string())
        );
        assert_eq!(
            finding.message,
            Some("using uninitialized data".to_string())
        );
        assert_eq!(
            finding.location.as_ref().unwrap().path.as_str(),
            "src/lib.rs"
        );
        assert_eq!(finding.location.as_ref().unwrap().line, Some(10));
    }

    #[test]
    fn test_convert_miri_json_with_error() {
        let json = r#"{"reason": "compiler-message", "message": {"code": {"code": "my_error", "explanation": "Error"}, "level": "error", "message": "Some error", "spans": [{"file_name": "src/main.rs", "line_start": 5, "line_end": 5, "column_start": 1, "column_end": 10}]}}
"#;

        let receipt = convert_miri_json(json, "cargo-miri").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Error);
        assert_eq!(finding.check_id, Some("miri.error".to_string()));
        assert_eq!(finding.message, Some("Some error".to_string()));
    }

    #[test]
    fn test_convert_miri_json_calculates_counts() {
        let json = r#"{"reason": "compiler-message", "message": {"code": {"code": "undef"}, "level": "error", "message": "Error 1", "spans": []}}
{"reason": "compiler-message", "message": {"code": {"code": "undef"}, "level": "error", "message": "Error 2", "spans": []}}
{"reason": "compiler-message", "message": {"code": {"code": "other"}, "level": "error", "message": "Error 3", "spans": []}}
"#;

        let receipt = convert_miri_json(json, "cargo-miri").unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.findings, 3);
        assert_eq!(receipt.verdict.counts.errors, 3);
        assert_eq!(receipt.verdict.counts.warnings, 0);
    }

    #[test]
    fn test_convert_miri_json_empty_passes() {
        let json = r#""#;

        let receipt = convert_miri_json(json, "cargo-miri").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_miri_json_skips_non_messages() {
        let json = r#"{"reason": "build-finished", "message": {"level": "note", "message": "Build finished", "spans": []}}
{"reason": "compiler-message", "message": {"code": {"code": "undef"}, "level": "error", "message": "Actual error", "spans": []}}
"#;

        let receipt = convert_miri_json(json, "cargo-miri").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(
            receipt.findings[0].message,
            Some("Actual error".to_string())
        );
    }
}
