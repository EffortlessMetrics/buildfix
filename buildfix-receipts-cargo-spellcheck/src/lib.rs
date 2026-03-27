use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoSpellcheckAdapter {
    sensor_id: String,
}

impl CargoSpellcheckAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-spellcheck".to_string(),
        }
    }
}

impl Default for CargoSpellcheckAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoSpellcheckAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_spellcheck_json(&content, &self.sensor_id)
    }
}

impl AdapterMetadata for CargoSpellcheckAdapter {
    fn name(&self) -> &str {
        "cargo-spellcheck"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-spellcheck.report.v1"]
    }
}

fn convert_spellcheck_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let report: SpellcheckReport = serde_json::from_str(content).map_err(AdapterError::Json)?;

    let mut findings = Vec::new();

    for finding in &report.findings {
        let check_id = if finding.kind.contains("Misspelled") {
            "docs.spelling_error".to_string()
        } else {
            "spellcheck.spelling".to_string()
        };

        let message = if let Some(ref suggestion) = finding.suggestion {
            format!(
                "Spelling error: '{}' (found in '{}'). Did you mean: '{}'?",
                finding.words.join(", "),
                finding.context,
                suggestion
            )
        } else {
            format!(
                "Spelling error: '{}' (found in '{}')",
                finding.words.join(", "),
                finding.context
            )
        };

        findings.push(Finding {
            severity: Severity::Warn,
            check_id: Some(check_id),
            code: None,
            message: Some(message),
            location: Some(Location {
                path: Utf8PathBuf::from(&finding.file),
                line: Some(finding.line),
                column: Some(finding.column),
            }),
            fingerprint: None,
            data: Some(serde_json::json!({
                "kind": finding.kind,
                "context": finding.context,
                "suggestion": finding.suggestion,
                "words": finding.words,
            })),
            ..Default::default()
        });
    }

    let status = if findings.is_empty() {
        VerdictStatus::Pass
    } else {
        VerdictStatus::Warn
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-spellcheck.findings.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, findings.len() as u64);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SpellcheckReport {
    findings: Vec<SpellcheckFinding>,
    summary: SpellcheckSummary,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SpellcheckFinding {
    file: String,
    line: u64,
    column: u64,
    kind: String,
    context: String,
    #[serde(default)]
    suggestion: Option<String>,
    words: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SpellcheckSummary {
    total: u64,
    files: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoSpellcheckAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-spellcheck");
    }

    #[test]
    fn test_convert_spellcheck_json() {
        let json = r#"{
  "findings": [
    {
      "file": "src/lib.rs",
      "line": 10,
      "column": 5,
      "kind": "Misspelled",
      "context": "This is a documint",
      "suggestion": "document",
      "words": ["documint"]
    }
  ],
  "summary": {
    "total": 1,
    "files": 1
  }
}"#;

        let receipt = convert_spellcheck_json(json, "cargo-spellcheck").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(finding.check_id, Some("docs.spelling_error".to_string()));
        assert!(finding.message.is_some());
        let location = finding.location.as_ref().unwrap();
        assert_eq!(location.path.as_str(), "src/lib.rs");
        assert_eq!(location.line, Some(10));
        assert_eq!(location.column, Some(5));
    }

    #[test]
    fn test_convert_spellcheck_json_with_multiple_findings() {
        let json = r#"{
  "findings": [
    {
      "file": "src/lib.rs",
      "line": 10,
      "column": 5,
      "kind": "Misspelled",
      "context": "This is a documint",
      "suggestion": "document",
      "words": ["documint"]
    },
    {
      "file": "src/main.rs",
      "line": 20,
      "column": 15,
      "kind": "Misspelled",
      "context": "functoin",
      "suggestion": "function",
      "words": ["functoin"]
    }
  ],
  "summary": {
    "total": 2,
    "files": 2
  }
}"#;

        let receipt = convert_spellcheck_json(json, "cargo-spellcheck").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_spellcheck_json_empty_passes() {
        let json = r#"{
  "findings": [],
  "summary": {
    "total": 0,
    "files": 0
  }
}"#;

        let receipt = convert_spellcheck_json(json, "cargo-spellcheck").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_spellcheck_json_without_suggestion() {
        let json = r#"{
  "findings": [
    {
      "file": "src/lib.rs",
      "line": 10,
      "column": 5,
      "kind": "UnknownWord",
      "context": "Some unknown word",
      "words": ["unknownword"]
    }
  ],
  "summary": {
    "total": 1,
    "files": 1
  }
}"#;

        let receipt = convert_spellcheck_json(json, "cargo-spellcheck").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.check_id, Some("spellcheck.spelling".to_string()));
    }

    #[test]
    fn test_load_from_file() {
        let adapter = CargoSpellcheckAdapter::new();
        let receipt = adapter
            .load(Path::new("tests/fixtures/report.json"))
            .expect("should load fixture");

        assert!(!receipt.findings.is_empty());
    }
}
