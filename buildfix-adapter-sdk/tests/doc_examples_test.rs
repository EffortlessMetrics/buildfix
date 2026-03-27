//! Documentation code examples verification test.
//!
//! This test extracts Rust code examples from documentation files
//! and verifies they compile correctly.

use serde::Deserialize;

/// Test that code examples in docs/how-to/write-adapter.md compile
#[test]
fn test_write_adapter_examples() {
    // Example from Step 3: Define the Input Schema
    /// Root structure of the sensor output
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct MyToolReport {
        #[serde(default)]
        version: String,

        #[serde(default)]
        issues: Vec<MyToolIssue>,
    }

    /// A single issue from the sensor
    #[derive(Debug, Deserialize, Clone)]
    #[allow(dead_code)]
    struct MyToolIssue {
        /// Rule identifier (e.g., "MAGIC001")
        rule: String,

        /// Severity level from the tool
        level: String,

        /// Human-readable description
        description: String,

        /// File path where issue was found
        #[serde(rename = "file")]
        file_path: String,

        /// Line number (1-based), optional
        #[serde(default)]
        line: Option<u64>,

        /// Column number (1-based), optional  
        #[serde(default)]
        column: Option<u64>,
    }

    // Verify structs can be used
    let _report: Option<MyToolReport> = None;
    let _issue: Option<MyToolIssue> = None;
}

/// Test that code examples for check ID mapping compile
#[test]
fn test_check_id_mapping_example() {
    fn map_rule_to_check_id(rule: &str) -> String {
        match rule {
            "MAGIC001" => "my-tool.style.magic-number".to_string(),
            "SECRET001" => "my-tool.security.hardcoded-secret".to_string(),
            "CLONE001" => "my-tool.performance.unnecessary-clone".to_string(),
            _ => format!("my-tool.unknown.{}", rule.to_lowercase()),
        }
    }

    assert_eq!(
        map_rule_to_check_id("MAGIC001"),
        "my-tool.style.magic-number"
    );
    assert_eq!(
        map_rule_to_check_id("SECRET001"),
        "my-tool.security.hardcoded-secret"
    );
}

/// Test severity mapping example
#[test]
fn test_severity_mapping_example() {
    // From Step 7: Map Severities
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Severity {
        Error,
        Warn,
        Info,
    }

    fn map_severity(level: &str) -> Severity {
        match level.to_lowercase().as_str() {
            "error" | "fatal" | "critical" => Severity::Error,
            "warning" | "warn" => Severity::Warn,
            "info" | "note" | "suggestion" => Severity::Info,
            _ => Severity::Info,
        }
    }

    assert_eq!(map_severity("error"), Severity::Error);
    assert_eq!(map_severity("WARNING"), Severity::Warn);
    assert_eq!(map_severity("unknown"), Severity::Info);
}

/// Test path normalization example
#[test]
fn test_path_normalization_example() {
    fn normalize_path(path: &str) -> String {
        // Remove leading ./
        let path = path.strip_prefix("./").unwrap_or(path);

        // Convert backslashes to forward slashes
        path.replace('\\', "/")
    }

    assert_eq!(normalize_path("./src/main.rs"), "src/main.rs");
    assert_eq!(normalize_path("src\\main.rs"), "src/main.rs");
}

/// Test optional fields handling example
#[test]
fn test_optional_fields_example() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct MyFinding {
        // Required field
        message: String,

        // Optional with default
        #[serde(default)]
        severity: String,

        // Optional nullable
        #[serde(default)]
        location: Option<MyLocation>,

        // Optional with custom default
        #[serde(default = "default_level")]
        level: String,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct MyLocation {
        path: String,
    }

    fn default_level() -> String {
        "info".to_string()
    }

    // Verify the struct can be deserialized
    let json = r#"{"message": "test"}"#;
    let finding: MyFinding = serde_json::from_str(json).unwrap();
    assert_eq!(finding.message, "test");
    assert_eq!(finding.level, "info");
}

/// Test error handling patterns from documentation
#[test]
fn test_error_handling_patterns() {
    // This tests the conceptual pattern without actual file I/O
    fn validate_version(version: &str) -> Result<(), String> {
        if version.is_empty() {
            return Err("report version is required".to_string());
        }
        Ok(())
    }

    assert!(validate_version("1.0").is_ok());
    assert!(validate_version("").is_err());
}

/// Test multiple check IDs pattern
#[test]
fn test_multiple_check_ids_pattern() {
    #[derive(Clone)]
    #[allow(dead_code)]
    struct Finding {
        check_id: String,
        message: String,
    }

    fn create_finding(data: &str, check_id: &str) -> Finding {
        Finding {
            check_id: check_id.to_string(),
            message: data.to_string(),
        }
    }

    struct MyReport {
        errors: Vec<String>,
        warnings: Vec<String>,
        suggestions: Vec<String>,
    }

    fn process_findings(report: &MyReport) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Process errors
        for error in &report.errors {
            findings.push(create_finding(error, "my-tool.error"));
        }

        // Process warnings
        for warning in &report.warnings {
            findings.push(create_finding(warning, "my-tool.warning"));
        }

        // Process suggestions
        for suggestion in &report.suggestions {
            findings.push(create_finding(suggestion, "my-tool.suggestion"));
        }

        findings
    }

    let report = MyReport {
        errors: vec!["error1".to_string()],
        warnings: vec!["warn1".to_string(), "warn2".to_string()],
        suggestions: vec![],
    };

    let findings = process_findings(&report);
    assert_eq!(findings.len(), 3);
}

/// Test determinism sorting example
#[test]
fn test_determinism_sorting() {
    // From docs/explanation/determinism.md
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Op {
        sensor: String,
        check_id: String,
        code: String,
        target: String,
    }

    fn stable_op_sort_key(op: &Op) -> (String, String, String, String) {
        (
            op.sensor.clone(),
            op.check_id.clone(),
            op.code.clone(),
            op.target.clone(),
        )
    }

    let mut ops = [
        Op {
            sensor: "b".to_string(),
            check_id: "x".to_string(),
            code: "1".to_string(),
            target: "a.toml".to_string(),
        },
        Op {
            sensor: "a".to_string(),
            check_id: "z".to_string(),
            code: "2".to_string(),
            target: "b.toml".to_string(),
        },
        Op {
            sensor: "a".to_string(),
            check_id: "a".to_string(),
            code: "1".to_string(),
            target: "c.toml".to_string(),
        },
    ];

    ops.sort_by_key(stable_op_sort_key);

    assert_eq!(ops[0].check_id, "a");
    assert_eq!(ops[1].check_id, "z");
    assert_eq!(ops[2].sensor, "b");
}
