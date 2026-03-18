use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, ReceiptBuilder};
use buildfix_types::receipt::{Finding, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct TarpaulinAdapter {
    sensor_id: String,
}

impl TarpaulinAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-tarpaulin".to_string(),
        }
    }
}

impl Default for TarpaulinAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for TarpaulinAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_tarpaulin_json(&content, &self.sensor_id)
    }
}

fn convert_tarpaulin_json(content: &str, sensor_id: &str) -> Result<ReceiptEnvelope, AdapterError> {
    let report: TarpaulinReport =
        serde_json::from_str(content).map_err(|e| AdapterError::InvalidFormat(e.to_string()))?;

    let mut findings = Vec::new();
    let mut warning_count = 0u64;

    let line_coverage_pct = if report.line_total > 0 {
        (report.line_covered as f64 / report.line_total as f64) * 100.0
    } else {
        0.0
    };

    let severity = if line_coverage_pct < 80.0 {
        warning_count += 1;
        Severity::Warn
    } else {
        Severity::Info
    };

    let check_id = if sensor_id == "cargo-tarpaulin" {
        "tarpaulin.low_coverage"
    } else {
        "coverage.low_coverage"
    };

    let message = format!(
        "Line coverage: {:.1}% ({}/{} lines covered)",
        line_coverage_pct, report.line_covered, report.line_total
    );

    findings.push(Finding {
        severity,
        check_id: Some(check_id.to_string()),
        code: None,
        message: Some(message),
        location: None,
        fingerprint: None,
        data: None,
    });

    if report.files.is_empty() {
        return Ok(build_receipt(sensor_id, findings, warning_count, 0));
    }

    for file in &report.files {
        let file_line_pct = if file.line_total > 0 {
            (file.line_covered as f64 / file.line_total as f64) * 100.0
        } else {
            0.0
        };

        let severity = if file_line_pct < 80.0 {
            warning_count += 1;
            Severity::Warn
        } else {
            Severity::Info
        };

        let check_id = if sensor_id == "cargo-tarpaulin" {
            "tarpaulin.low_coverage"
        } else {
            "coverage.low_coverage"
        };

        let message = format!(
            "File {}: {:.1}% line coverage ({}/{} lines)",
            file.path, file_line_pct, file.line_covered, file.line_total
        );

        findings.push(Finding {
            severity,
            check_id: Some(check_id.to_string()),
            code: None,
            message: Some(message),
            location: Some(buildfix_types::receipt::Location {
                path: Utf8PathBuf::from(&file.path),
                line: None,
                column: None,
            }),
            fingerprint: None,
            data: None,
        });
    }

    Ok(build_receipt(sensor_id, findings, warning_count, 0))
}

fn build_receipt(
    sensor_id: &str,
    findings: Vec<Finding>,
    warnings: u64,
    _errors: u64,
) -> ReceiptEnvelope {
    let status = if warnings > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("tarpaulin.report.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, warnings);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    builder.build()
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct TarpaulinReport {
    #[serde(default)]
    files: Vec<TarpaulinFile>,
    #[serde(default)]
    line_covered: u64,
    #[serde(default)]
    line_total: u64,
    #[serde(default)]
    line_uncovered: u64,
    #[serde(default)]
    branch_covered: u64,
    #[serde(default)]
    branch_total: u64,
    #[serde(default)]
    branch_uncovered: u64,
    #[serde(default)]
    functions_covered: u64,
    #[serde(default)]
    functions_total: u64,
    #[serde(default)]
    functions_uncovered: u64,
    #[serde(default)]
    uncovered_lines: Vec<u64>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct TarpaulinFile {
    #[serde(default)]
    path: String,
    #[serde(default)]
    line_covered: u64,
    #[serde(default)]
    line_uncovered: u64,
    #[serde(default)]
    line_total: u64,
    #[serde(default)]
    branch_covered: u64,
    #[serde(default)]
    branch_uncovered: u64,
    #[serde(default)]
    branch_total: u64,
    #[serde(default)]
    functions_covered: u64,
    #[serde(default)]
    functions_uncovered: u64,
    #[serde(default)]
    functions_total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = TarpaulinAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-tarpaulin");
    }

    #[test]
    fn test_convert_tarpaulin_json_high_coverage() {
        let json = r#"{
            "files": [
                {
                    "path": "src/lib.rs",
                    "line_covered": 90,
                    "line_uncovered": 10,
                    "line_total": 100,
                    "branch_covered": 20,
                    "branch_uncovered": 5,
                    "branch_total": 25,
                    "functions_covered": 8,
                    "functions_uncovered": 2,
                    "functions_total": 10
                }
            ],
            "line_covered": 900,
            "line_total": 1000,
            "branch_covered": 200,
            "branch_total": 250,
            "functions_covered": 80,
            "functions_total": 100,
            "uncovered_lines": [10, 20, 30]
        }"#;

        let receipt = convert_tarpaulin_json(json, "cargo-tarpaulin").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        let overall = &receipt.findings[0];
        assert_eq!(overall.severity, Severity::Info);
        assert_eq!(overall.check_id, Some("tarpaulin.low_coverage".to_string()));
    }

    #[test]
    fn test_convert_tarpaulin_json_low_coverage() {
        let json = r#"{
            "files": [
                {
                    "path": "src/lib.rs",
                    "line_covered": 50,
                    "line_uncovered": 10,
                    "line_total": 60,
                    "branch_covered": 20,
                    "branch_uncovered": 5,
                    "branch_total": 25,
                    "functions_covered": 8,
                    "functions_uncovered": 2,
                    "functions_total": 10
                }
            ],
            "line_covered": 500,
            "line_total": 1000,
            "branch_covered": 200,
            "branch_total": 250,
            "functions_covered": 80,
            "functions_total": 100,
            "uncovered_lines": [10, 20, 30]
        }"#;

        let receipt = convert_tarpaulin_json(json, "cargo-tarpaulin").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        let overall = &receipt.findings[0];
        assert_eq!(overall.severity, Severity::Warn);
        assert!(overall.message.as_ref().unwrap().contains("50.0%"));
    }

    #[test]
    fn test_convert_tarpaulin_json_empty_files() {
        let json = r#"{
            "files": [],
            "line_covered": 0,
            "line_total": 0,
            "branch_covered": 0,
            "branch_total": 0,
            "functions_covered": 0,
            "functions_total": 0,
            "uncovered_lines": []
        }"#;

        let receipt = convert_tarpaulin_json(json, "cargo-tarpaulin").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }

    #[test]
    fn test_convert_tarpaulin_json_check_id_format() {
        let json = r#"{
            "files": [],
            "line_covered": 500,
            "line_total": 1000,
            "branch_covered": 200,
            "branch_total": 250,
            "functions_covered": 80,
            "functions_total": 100,
            "uncovered_lines": []
        }"#;

        let receipt = convert_tarpaulin_json(json, "cargo-tarpaulin").unwrap();
        assert_eq!(
            receipt.findings[0].check_id,
            Some("tarpaulin.low_coverage".to_string())
        );

        let receipt2 = convert_tarpaulin_json(json, "other-sensor").unwrap();
        assert_eq!(
            receipt2.findings[0].check_id,
            Some("coverage.low_coverage".to_string())
        );
    }

    #[test]
    fn test_file_level_findings() {
        let json = r#"{
            "files": [
                {
                    "path": "src/lib.rs",
                    "line_covered": 50,
                    "line_uncovered": 10,
                    "line_total": 60,
                    "branch_covered": 20,
                    "branch_uncovered": 5,
                    "branch_total": 25,
                    "functions_covered": 8,
                    "functions_uncovered": 2,
                    "functions_total": 10
                },
                {
                    "path": "src/main.rs",
                    "line_covered": 90,
                    "line_uncovered": 10,
                    "line_total": 100,
                    "branch_covered": 20,
                    "branch_uncovered": 5,
                    "branch_total": 25,
                    "functions_covered": 8,
                    "functions_uncovered": 2,
                    "functions_total": 10
                }
            ],
            "line_covered": 500,
            "line_total": 1000,
            "branch_covered": 200,
            "branch_total": 250,
            "functions_covered": 80,
            "functions_total": 100,
            "uncovered_lines": [10, 20, 30]
        }"#;

        let receipt = convert_tarpaulin_json(json, "cargo-tarpaulin").unwrap();

        assert_eq!(receipt.findings.len(), 3);

        let file1 = &receipt.findings[1];
        assert!(file1.message.as_ref().unwrap().contains("src/lib.rs"));
        assert_eq!(file1.severity, Severity::Info);

        let file2 = &receipt.findings[2];
        assert!(file2.message.as_ref().unwrap().contains("src/main.rs"));
        assert_eq!(file2.severity, Severity::Info);
    }

    #[test]
    fn test_invalid_json() {
        let json = r#"not valid json"#;

        let result = convert_tarpaulin_json(json, "cargo-tarpaulin");
        assert!(result.is_err());
    }
}
