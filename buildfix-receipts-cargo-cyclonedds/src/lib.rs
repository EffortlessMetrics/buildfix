use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

pub struct CargoHackAdapter {
    sensor_id: String,
}

impl CargoHackAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "cargo-hack".to_string(),
        }
    }
}

impl Default for CargoHackAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CargoHackAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        convert_cargo_hack_json(&content, &self.sensor_id)
    }
}

impl AdapterMetadata for CargoHackAdapter {
    fn name(&self) -> &str {
        "cargo-cyclonedds"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-cyclonedds.report.v1"]
    }
}

fn convert_cargo_hack_json(
    content: &str,
    sensor_id: &str,
) -> Result<ReceiptEnvelope, AdapterError> {
    let reports: Vec<CargoHackReport> =
        serde_json::from_str(content).map_err(|e| AdapterError::InvalidFormat(e.to_string()))?;

    let mut findings = Vec::new();
    let mut warning_count = 0u64;

    for report in &reports {
        for unstable in &report.unstable_features {
            warning_count += 1;

            let message = if let Some(usage) = &unstable.usage {
                format!("unstable feature '{}' used: {}", unstable.feature, usage)
            } else {
                format!("unstable feature '{}' used", unstable.feature)
            };

            let location = extract_location_from_usage(&unstable.usage);

            findings.push(Finding {
                severity: Severity::Warn,
                check_id: Some("hack.unstable".to_string()),
                code: Some(unstable.feature.clone()),
                message: Some(message),
                location,
                fingerprint: None,
                data: Some(serde_json::json!({
                    "package": report.package,
                    "features": report.features,
                    "feature": unstable.feature,
                })),
                ..Default::default()
            });
        }
    }

    let status = if warning_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new(sensor_id)
        .with_schema("cargo-hack.report.v1")
        .with_status(status)
        .with_counts(findings.len() as u64, 0, warning_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

fn extract_location_from_usage(
    usage: &Option<String>,
) -> Option<buildfix_types::receipt::Location> {
    let usage = usage.as_ref()?;

    let line_number = usage.lines().enumerate().find_map(|(idx, line)| {
        if line.contains("fn ") || line.contains("struct ") || line.contains("impl ") {
            Some((idx + 1) as u64)
        } else {
            None
        }
    });

    Some(buildfix_types::receipt::Location {
        path: Utf8PathBuf::from("src/lib.rs"),
        line: line_number,
        column: None,
    })
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct CargoHackReport {
    #[serde(default)]
    package: String,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    unstable_features: Vec<UnstableFeature>,
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
struct UnstableFeature {
    #[serde(default)]
    feature: String,
    #[serde(default)]
    usage: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_sensor_id() {
        let adapter = CargoHackAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-hack");
    }

    #[test]
    fn test_convert_cargo_hack_json_with_unstable() {
        let json = r#"[
  {
    "package": "my-crate",
    "features": ["default", "full"],
    "unstable_features": [
      {
        "feature": "generic_const_exprs",
        "usage": "impl<T> Foo<T> { ... }"
      }
    ]
  }
]"#;

        let receipt = convert_cargo_hack_json(json, "cargo-hack").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Warn);
        assert_eq!(finding.check_id, Some("hack.unstable".to_string()));
        assert_eq!(finding.code, Some("generic_const_exprs".to_string()));
        assert!(
            finding
                .message
                .as_ref()
                .unwrap()
                .contains("generic_const_exprs")
        );
    }

    #[test]
    fn test_convert_cargo_hack_json_multiple_packages() {
        let json = r#"[
  {
    "package": "crate-a",
    "features": ["default"],
    "unstable_features": [
      { "feature": "feature_a", "usage": "fn foo() { }" }
    ]
  },
  {
    "package": "crate-b",
    "features": ["default"],
    "unstable_features": [
      { "feature": "feature_b" }
    ]
  }
]"#;

        let receipt = convert_cargo_hack_json(json, "cargo-hack").unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_convert_cargo_hack_json_empty_passes() {
        let json = r#"[]"#;

        let receipt = convert_cargo_hack_json(json, "cargo-hack").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_hack_json_no_unstable_features() {
        let json = r#"[
  {
    "package": "my-crate",
    "features": ["default"],
    "unstable_features": []
  }
]"#;

        let receipt = convert_cargo_hack_json(json, "cargo-hack").unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_convert_cargo_hack_json_unstable_without_usage() {
        let json = r#"[
  {
    "package": "my-crate",
    "features": ["default"],
    "unstable_features": [
      { "feature": "asm" }
    ]
  }
]"#;

        let receipt = convert_cargo_hack_json(json, "cargo-hack").unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert!(
            finding
                .message
                .as_ref()
                .unwrap()
                .contains("unstable feature 'asm' used")
        );
    }
}
