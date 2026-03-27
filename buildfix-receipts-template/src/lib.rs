//! Template adapter for buildfix - demonstrates adapter development patterns.
//!
//! This crate serves as a complete example for developers creating new adapters.
//! It implements a fictional "example-linter" tool adapter that demonstrates all
//! best practices and patterns used in the buildfix adapter ecosystem.
//!
//! # Overview
//!
//! An adapter transforms a sensor's native output format into the standardized
//! `ReceiptEnvelope` format that buildfix expects. This template shows:
//!
//! - Implementing the [`Adapter`] trait for loading and parsing sensor output
//! - Implementing the [`AdapterMetadata`] trait for self-description
//! - Proper error handling with [`AdapterError`]
//! - Check ID mapping conventions
//! - Severity mapping from tool-specific to buildfix standard
//! - Using the [`ReceiptBuilder`] pattern for constructing receipts
//!
//! # Creating a New Adapter
//!
//! To create a new adapter based on this template:
//!
//! 1. Copy this crate directory and rename it (e.g., `buildfix-receipts-mytool`)
//! 2. Update `Cargo.toml` with your tool's name and description
//! 3. Rename the adapter struct (e.g., `MyToolAdapter`)
//! 4. Update the input types to match your tool's JSON schema
//! 5. Implement the conversion logic in `convert_report`
//! 6. Update the check ID mapping table in CLAUDE.md
//! 7. Add test fixtures that match your tool's output format
//!
//! # Example Input Format
//!
//! This template adapter expects JSON input in the following format:
//!
//! ```json
//! {
//!   "version": "1.0",
//!   "findings": [
//!     {
//!       "rule": "EXAMPLE001",
//!       "severity": "error",
//!       "message": "Example finding",
//!       "file": "src/main.rs",
//!       "line": 42
//!     }
//!   ]
//! }
//! ```

use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

/// Adapter for the example-linter tool.
///
/// This adapter parses JSON output from the fictional "example-linter" tool
/// and converts it to the buildfix receipt format.
///
/// # Example
///
/// ```
/// use buildfix_adapter_sdk::Adapter;
/// use buildfix_receipts_template::ExampleLinterAdapter;
/// use std::path::Path;
///
/// let adapter = ExampleLinterAdapter::new();
/// assert_eq!(adapter.sensor_id(), "example-linter");
/// ```
pub struct ExampleLinterAdapter {
    /// The sensor identifier for this adapter.
    sensor_id: &'static str,
}

impl ExampleLinterAdapter {
    /// Creates a new instance of the example-linter adapter.
    ///
    /// # Example
    ///
    /// ```
    /// use buildfix_receipts_template::ExampleLinterAdapter;
    ///
    /// let adapter = ExampleLinterAdapter::new();
    /// ```
    pub fn new() -> Self {
        Self {
            sensor_id: "example-linter",
        }
    }
}

impl Default for ExampleLinterAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for ExampleLinterAdapter {
    /// Returns the unique sensor identifier for this adapter.
    ///
    /// This identifier is used to route findings to the appropriate fixers
    /// and must match the sensor directory name in the artifacts folder.
    fn sensor_id(&self) -> &str {
        self.sensor_id
    }

    /// Loads and parses the example-linter output from a file.
    ///
    /// This method:
    /// 1. Reads the file content
    /// 2. Parses the JSON into the tool-specific format
    /// 3. Converts it to the standardized `ReceiptEnvelope`
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::Io`] if the file cannot be read.
    /// Returns [`AdapterError::Json`] if the JSON is malformed.
    /// Returns [`AdapterError::InvalidFormat`] if the JSON structure is invalid.
    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: ExampleLinterReport =
            serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

impl AdapterMetadata for ExampleLinterAdapter {
    /// Returns the adapter name.
    ///
    /// This should be a unique, stable identifier matching the sensor tool name.
    fn name(&self) -> &str {
        "example-linter"
    }

    /// Returns the adapter version using the crate's version.
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    /// Returns the list of schema versions this adapter supports.
    ///
    /// Format: "sensor.report.v1" style strings.
    fn supported_schemas(&self) -> &[&str] {
        &["example-linter.report.v1"]
    }
}

// ============================================================================
// Input Types - These define the structure of your tool's JSON output
// ============================================================================

/// The root structure of the example-linter JSON output.
///
/// When creating your own adapter, define types that match your tool's
/// output format. Use `#[serde(default)]` for optional fields and
/// `#[serde(rename = "...")]` for field name mapping.
#[derive(Debug, Deserialize)]
struct ExampleLinterReport {
    /// The version of the report format.
    #[serde(default)]
    version: String,

    /// The list of findings from the linter.
    #[serde(default)]
    findings: Vec<ExampleLinterFinding>,
}

/// A single finding from the example-linter.
///
/// Each finding represents one issue detected by the tool.
#[derive(Debug, Deserialize, Clone)]
struct ExampleLinterFinding {
    /// The rule ID that was violated (e.g., "EXAMPLE001").
    rule: String,

    /// The severity level: "error", "warning", or "info".
    severity: String,

    /// Human-readable message describing the finding.
    message: String,

    /// The file path where the finding was detected.
    #[serde(rename = "file")]
    file_path: String,

    /// The line number where the finding was detected (1-based).
    #[serde(default)]
    line: Option<u64>,

    /// The column number where the finding was detected (1-based).
    #[serde(default)]
    column: Option<u64>,
}

// ============================================================================
// Conversion Logic - Transform tool output to buildfix receipt
// ============================================================================

/// Converts the tool-specific report to a buildfix receipt envelope.
///
/// This function is where you implement the core transformation logic:
///
/// 1. Iterate through findings from the tool
/// 2. Map each finding to a buildfix `Finding` struct
/// 3. Map severity levels to buildfix `Severity`
/// 4. Generate check IDs following the naming convention
/// 5. Build the receipt using `ReceiptBuilder`
fn convert_report(report: ExampleLinterReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut error_count = 0u64;
    let mut warn_count = 0u64;

    for finding in &report.findings {
        // Map the tool's severity to buildfix severity
        let severity = map_severity(&finding.severity);

        // Generate a check ID following the naming convention
        // Format: <tool>.<category>.<specific>
        let check_id = format_check_id(&finding.rule);

        // Create the location with normalized path
        let location = Location {
            path: normalize_path(&finding.file_path),
            line: finding.line,
            column: finding.column,
        };

        // Build the finding
        let receipt_finding = Finding {
            severity,
            check_id: Some(check_id.clone()),
            code: Some(finding.rule.clone()),
            message: Some(finding.message.clone()),
            location: Some(location),
            fingerprint: None,
            data: None,
            ..Default::default()
        };

        findings.push(receipt_finding);

        // Track counts by severity
        match severity {
            Severity::Error => error_count += 1,
            Severity::Warn => warn_count += 1,
            Severity::Info => {}
        }
    }

    // Determine overall status based on findings
    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    // Build the receipt using the builder pattern
    let mut builder = ReceiptBuilder::new("example-linter")
        .with_schema("example-linter.report.v1")
        .with_tool_version(report.version)
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, warn_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

/// Maps the tool's severity string to buildfix `Severity`.
///
/// When creating your adapter, implement a similar function to translate
/// your tool's severity levels to the standard buildfix levels.
fn map_severity(severity: &str) -> Severity {
    match severity.to_lowercase().as_str() {
        "error" | "err" | "fatal" | "critical" => Severity::Error,
        "warning" | "warn" | "major" => Severity::Warn,
        _ => Severity::Info,
    }
}

/// Formats a check ID following the buildfix naming convention.
///
/// Check IDs should follow the format: `<tool>.<category>.<specific>`
///
/// Examples:
/// - `example-linter.style.EXAMPLE001`
/// - `example-linter.correctness.EXAMPLE002`
fn format_check_id(rule: &str) -> String {
    // In a real adapter, you might parse the rule to determine category
    // For this template, we use a default category
    format!("example-linter.code.{}", rule)
}

/// Normalizes a file path for consistent handling.
///
/// Path normalization ensures:
/// - Forward slashes are used (cross-platform)
/// - No leading `./`
/// - Relative to repository root when possible
fn normalize_path(path: &str) -> Utf8PathBuf {
    // Remove leading "./" if present
    let path = path.strip_prefix("./").unwrap_or(path);

    // Convert to forward slashes
    Utf8PathBuf::from(path.replace('\\', "/"))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = ExampleLinterAdapter::new();
        assert_eq!(adapter.sensor_id(), "example-linter");
    }

    #[test]
    fn test_adapter_metadata() {
        let adapter = ExampleLinterAdapter::new();
        assert_eq!(adapter.name(), "example-linter");
        assert!(!adapter.version().is_empty());
        assert_eq!(adapter.supported_schemas(), &["example-linter.report.v1"]);
    }

    #[test]
    fn test_convert_report_with_findings() {
        let json = r#"{
            "version": "1.0",
            "findings": [
                {
                    "rule": "EXAMPLE001",
                    "severity": "error",
                    "message": "Example error finding",
                    "file": "src/main.rs",
                    "line": 42,
                    "column": 10
                },
                {
                    "rule": "EXAMPLE002",
                    "severity": "warning",
                    "message": "Example warning finding",
                    "file": "src/lib.rs",
                    "line": 10
                }
            ]
        }"#;

        let report: ExampleLinterReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.errors, 1);
        assert_eq!(receipt.verdict.counts.warnings, 1);

        let finding1 = &receipt.findings[0];
        assert_eq!(finding1.severity, Severity::Error);
        assert_eq!(
            finding1.check_id,
            Some("example-linter.code.EXAMPLE001".to_string())
        );
        assert_eq!(finding1.code, Some("EXAMPLE001".to_string()));
        assert_eq!(finding1.message, Some("Example error finding".to_string()));
        assert!(finding1.location.is_some());
        let loc1 = finding1.location.as_ref().unwrap();
        assert_eq!(loc1.path.as_str(), "src/main.rs");
        assert_eq!(loc1.line, Some(42));
        assert_eq!(loc1.column, Some(10));
    }

    #[test]
    fn test_convert_report_empty_passes() {
        let json = r#"{
            "version": "1.0",
            "findings": []
        }"#;

        let report: ExampleLinterReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_report_warning_status() {
        let json = r#"{
            "version": "1.0",
            "findings": [
                {
                    "rule": "EXAMPLE003",
                    "severity": "warning",
                    "message": "Warning only",
                    "file": "src/lib.rs"
                }
            ]
        }"#;

        let report: ExampleLinterReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }

    #[test]
    fn test_map_severity() {
        assert_eq!(map_severity("error"), Severity::Error);
        assert_eq!(map_severity("ERROR"), Severity::Error);
        assert_eq!(map_severity("fatal"), Severity::Error);
        assert_eq!(map_severity("warning"), Severity::Warn);
        assert_eq!(map_severity("WARN"), Severity::Warn);
        assert_eq!(map_severity("info"), Severity::Info);
        assert_eq!(map_severity("note"), Severity::Info);
        assert_eq!(map_severity("unknown"), Severity::Info);
    }

    #[test]
    fn test_format_check_id() {
        assert_eq!(
            format_check_id("EXAMPLE001"),
            "example-linter.code.EXAMPLE001"
        );
        assert_eq!(format_check_id("RULE42"), "example-linter.code.RULE42");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(
            normalize_path("src/main.rs"),
            Utf8PathBuf::from("src/main.rs")
        );
        assert_eq!(
            normalize_path("./src/main.rs"),
            Utf8PathBuf::from("src/main.rs")
        );
        assert_eq!(
            normalize_path("src\\main.rs"),
            Utf8PathBuf::from("src/main.rs")
        );
        assert_eq!(
            normalize_path("./src\\main.rs"),
            Utf8PathBuf::from("src/main.rs")
        );
    }

    #[test]
    fn test_default_implementation() {
        let adapter = ExampleLinterAdapter::default();
        assert_eq!(adapter.sensor_id(), "example-linter");
    }
}
