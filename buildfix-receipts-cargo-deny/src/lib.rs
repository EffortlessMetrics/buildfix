use anyhow::Result;
use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct CargoDenyAdapter;

impl CargoDenyAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CargoDenyAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AdapterMetadata for CargoDenyAdapter {
    fn name(&self) -> &str {
        "cargo-deny"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["cargo-deny.report.v1"]
    }
}

impl Adapter for CargoDenyAdapter {
    fn sensor_id(&self) -> &str {
        "cargo-deny"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: CargoDenyReport = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

fn convert_report(report: CargoDenyReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut deny_count = 0u64;
    let mut warn_count = 0u64;

    if let Some(licenses) = &report.licenses {
        process_license_section(licenses, &mut findings);
    }

    if let Some(bans) = &report.bans {
        process_bans_section(bans, &mut findings);
    }

    if let Some(advisories) = &report.advisories {
        process_advisories_section(advisories, &mut findings);
    }

    if let Some(sources) = &report.sources {
        process_sources_section(sources, &mut findings);
    }

    for finding in &findings {
        match finding.severity {
            Severity::Error => deny_count += 1,
            Severity::Warn => warn_count += 1,
            _ => {}
        }
    }

    let status = if deny_count > 0 {
        VerdictStatus::Fail
    } else if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new("cargo-deny")
        .with_schema("cargo-deny.report.v1")
        .with_tool_version("0.0.0")
        .with_status(status)
        .with_counts(findings.len() as u64, deny_count, warn_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    let receipt = builder.build();

    Ok(receipt)
}

#[derive(Debug, Clone, Default, Serialize)]
struct FindingsData {
    name: Option<String>,
    package: Option<PackageInfo>,
    features: Option<Vec<String>>,
    source: Option<String>,
    advisory_id: Option<String>,
}

fn process_license_section(licenses: &LicenseSection, findings: &mut Vec<Finding>) {
    for deny in &licenses.deny {
        let check_id = match &deny.id {
            Some(id) if id.contains("missing") => "licenses.missing",
            Some(id) if id.contains("unlicensed") => "licenses.unlicensed",
            _ => "licenses.unlicensed",
        };

        let data = FindingsData {
            name: deny.license.clone(),
            package: deny.package.clone(),
            features: None,
            source: None,
            advisory_id: None,
        };

        findings.push(Finding {
            severity: Severity::Error,
            check_id: Some(check_id.to_string()),
            code: deny.id.clone(),
            message: Some(deny.message.clone()),
            location: parse_cargo_deny_location(&deny.license),
            fingerprint: None,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            ..Default::default()
        });
    }

    for warn in &licenses.warn {
        let check_id = match &warn.id {
            Some(id) if id.contains("missing") => "licenses.missing",
            Some(id) if id.contains("unlicensed") => "licenses.unlicensed",
            _ => "licenses.unlicensed",
        };

        let data = FindingsData {
            name: warn.license.clone(),
            package: warn.package.clone(),
            features: None,
            source: None,
            advisory_id: None,
        };

        findings.push(Finding {
            severity: Severity::Warn,
            check_id: Some(check_id.to_string()),
            code: warn.id.clone(),
            message: Some(warn.message.clone()),
            location: parse_cargo_deny_location(&warn.license),
            fingerprint: None,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            ..Default::default()
        });
    }
}

fn process_bans_section(bans: &BansSection, findings: &mut Vec<Finding>) {
    for deny in &bans.deny {
        let check_id = map_bans_check_id(&deny.id);

        let data = FindingsData {
            name: None,
            package: deny.package.clone(),
            features: deny.features.clone(),
            source: None,
            advisory_id: None,
        };

        findings.push(Finding {
            severity: Severity::Error,
            check_id: Some(check_id),
            code: deny.id.clone(),
            message: Some(deny.message.clone()),
            location: parse_bans_location(&deny.package),
            fingerprint: None,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            ..Default::default()
        });
    }

    for warn in &bans.warn {
        let check_id = map_bans_check_id(&warn.id);

        let data = FindingsData {
            name: None,
            package: warn.package.clone(),
            features: warn.features.clone(),
            source: None,
            advisory_id: None,
        };

        findings.push(Finding {
            severity: Severity::Warn,
            check_id: Some(check_id),
            code: warn.id.clone(),
            message: Some(warn.message.clone()),
            location: parse_bans_location(&warn.package),
            fingerprint: None,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            ..Default::default()
        });
    }
}

fn map_bans_check_id(id: &Option<String>) -> String {
    match id.as_deref() {
        Some("multi-usage") => "bans.multi-usage".to_string(),
        Some("circular") => "bans.circular".to_string(),
        Some("multiple-versions") => "bans.multiple-versions".to_string(),
        Some("wildcard-dependencies") => "bans.wildcard-dependencies".to_string(),
        Some("all") => "bans.all".to_string(),
        Some("allow-warnings") => "bans.allow-warnings".to_string(),
        Some("deny-warnings") => "bans.deny-warnings".to_string(),
        Some(id) => format!("bans.{}", id),
        None => "bans.unknown".to_string(),
    }
}

fn parse_bans_location(pkg: &Option<PackageInfo>) -> Option<Location> {
    pkg.as_ref().map(|p| Location {
        path: Utf8PathBuf::from(format!("{}/Cargo.toml", p.name.replace('-', "_"))),
        line: None,
        column: None,
    })
}

fn process_advisories_section(advisories: &AdvisoriesSection, findings: &mut Vec<Finding>) {
    for deny in &advisories.deny {
        let data = FindingsData {
            name: None,
            package: deny.package.clone(),
            features: None,
            source: None,
            advisory_id: deny.advisory_id.clone(),
        };

        findings.push(Finding {
            severity: Severity::Error,
            check_id: Some(format!(
                "advisories.{}",
                deny.id.as_deref().unwrap_or("vulnerability")
            )),
            code: deny.id.clone(),
            message: Some(deny.message.clone()),
            location: None,
            fingerprint: None,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            ..Default::default()
        });
    }

    for warn in &advisories.warn {
        let data = FindingsData {
            name: None,
            package: warn.package.clone(),
            features: None,
            source: None,
            advisory_id: warn.advisory_id.clone(),
        };

        findings.push(Finding {
            severity: Severity::Warn,
            check_id: Some(format!(
                "advisories.{}",
                warn.id.as_deref().unwrap_or("warning")
            )),
            code: warn.id.clone(),
            message: Some(warn.message.clone()),
            location: None,
            fingerprint: None,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            ..Default::default()
        });
    }
}

fn process_sources_section(sources: &SourcesSection, findings: &mut Vec<Finding>) {
    for deny in &sources.deny {
        let data = FindingsData {
            name: None,
            package: None,
            features: None,
            source: deny.source.clone(),
            advisory_id: None,
        };

        findings.push(Finding {
            severity: Severity::Error,
            check_id: Some("sources.untrusted".to_string()),
            code: deny.id.clone(),
            message: Some(deny.message.clone()),
            location: None,
            fingerprint: None,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            ..Default::default()
        });
    }

    for warn in &sources.warn {
        let data = FindingsData {
            name: None,
            package: None,
            features: None,
            source: warn.source.clone(),
            advisory_id: None,
        };

        findings.push(Finding {
            severity: Severity::Warn,
            check_id: Some("sources.untrusted".to_string()),
            code: warn.id.clone(),
            message: Some(warn.message.clone()),
            location: None,
            fingerprint: None,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            ..Default::default()
        });
    }
}

fn parse_cargo_deny_location(license: &Option<String>) -> Option<Location> {
    license.as_ref().map(|l| Location {
        path: Utf8PathBuf::from(format!("{}/Cargo.toml", l.replace('-', "_"))),
        line: Some(1),
        column: Some(1),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CargoDenyReport {
    #[serde(default)]
    advisories: Option<AdvisoriesSection>,
    #[serde(default)]
    bans: Option<BansSection>,
    #[serde(default)]
    licenses: Option<LicenseSection>,
    #[serde(default)]
    sources: Option<SourcesSection>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct AdvisoriesSection {
    #[serde(default)]
    deny: Vec<AdvisoriesDeny>,
    #[serde(default)]
    warn: Vec<AdvisoriesWarn>,
    #[serde(default)]
    #[allow(dead_code)]
    skip: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct AdvisoriesDeny {
    id: Option<String>,
    message: String,
    advisory_id: Option<String>,
    package: Option<PackageInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct AdvisoriesWarn {
    id: Option<String>,
    message: String,
    advisory_id: Option<String>,
    package: Option<PackageInfo>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct BansSection {
    #[serde(default)]
    deny: Vec<BansDeny>,
    #[serde(default)]
    warn: Vec<BansWarn>,
    #[serde(default)]
    #[allow(dead_code)]
    skip: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct BansDeny {
    id: Option<String>,
    message: String,
    package: Option<PackageInfo>,
    features: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct BansWarn {
    id: Option<String>,
    message: String,
    package: Option<PackageInfo>,
    features: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct LicenseSection {
    #[serde(default)]
    deny: Vec<LicenseDeny>,
    #[serde(default)]
    warn: Vec<LicenseWarn>,
    #[serde(default)]
    #[allow(dead_code)]
    skip: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LicenseDeny {
    id: Option<String>,
    message: String,
    license: Option<String>,
    package: Option<PackageInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LicenseWarn {
    id: Option<String>,
    message: String,
    license: Option<String>,
    package: Option<PackageInfo>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct SourcesSection {
    #[serde(default)]
    deny: Vec<SourcesDeny>,
    #[serde(default)]
    warn: Vec<SourcesWarn>,
    #[serde(default)]
    #[allow(dead_code)]
    skip: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SourcesDeny {
    id: Option<String>,
    message: String,
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SourcesWarn {
    id: Option<String>,
    message: String,
    source: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default, Serialize)]
#[serde(rename_all = "kebab-case")]
struct PackageInfo {
    name: String,
    version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_loads_report() {
        let adapter = CargoDenyAdapter::new();
        assert_eq!(adapter.sensor_id(), "cargo-deny");
    }

    #[test]
    fn test_convert_report_with_license_findings() {
        let json = r#"{
            "licenses": {
                "deny": [
                    {
                        "id": "missing-license",
                        "message": "failed to detect a valid SPDX license expression",
                        "license": "crate-a"
                    }
                ]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Error);
        assert_eq!(finding.check_id, Some("licenses.missing".to_string()));
    }

    #[test]
    fn test_convert_report_with_bans_findings() {
        let json = r#"{
            "bans": {
                "deny": [
                    {
                        "id": "multi-usage",
                        "message": "package chrono is used multiple times",
                        "package": {
                            "name": "chrono",
                            "version": "0.4.31"
                        }
                    }
                ]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        let finding = &receipt.findings[0];
        assert_eq!(finding.severity, Severity::Error);
        assert_eq!(finding.check_id, Some("bans.multi-usage".to_string()));
    }

    #[test]
    fn test_convert_report_calculates_correct_counts() {
        let json = r#"{
            "licenses": {
                "deny": [{"id": "unlicensed", "message": "unlicensed", "license": "a"}],
                "warn": [{"id": "unlicensed", "message": "unlicensed", "license": "b"}]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.findings, 2);
        assert_eq!(receipt.verdict.counts.errors, 1);
        assert_eq!(receipt.verdict.counts.warnings, 1);
    }

    #[test]
    fn test_convert_report_empty_passes() {
        let json = r#"{}"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_adapter_loads_from_file_fixture() {
        let adapter = CargoDenyAdapter::new();
        let path = Path::new("tests/fixtures/report.json");
        let result = adapter.load(path);

        assert!(result.is_ok(), "should load fixture file");
        let receipt = result.unwrap();
        assert_eq!(receipt.findings.len(), 7);
    }

    #[test]
    fn test_adapter_returns_error_for_missing_file() {
        let adapter = CargoDenyAdapter::new();
        let path = Path::new("tests/fixtures/nonexistent.json");
        let result = adapter.load(path);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Io(_)));
    }

    #[test]
    fn test_adapter_returns_error_for_invalid_json() {
        let adapter = CargoDenyAdapter::new();
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("invalid_cargo_deny.json");
        std::fs::write(&temp_file, "{ invalid json }").unwrap();

        let result = adapter.load(&temp_file);
        std::fs::remove_file(&temp_file).ok();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AdapterError::Json(_)));
    }

    #[test]
    fn test_severity_mapping_error() {
        let json = r#"{
            "licenses": {
                "deny": [{"id": "unlicensed", "message": "no license", "license": "foo"}]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Error);
    }

    #[test]
    fn test_severity_mapping_warn() {
        let json = r#"{
            "licenses": {
                "warn": [{"id": "missing-license", "message": "license missing", "license": "bar"}]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Warn);
    }

    #[test]
    fn test_verdict_calculation_with_only_warnings() {
        let json = r#"{
            "licenses": {
                "warn": [
                    {"id": "missing-license", "message": "warning 1", "license": "a"},
                    {"id": "missing-license", "message": "warning 2", "license": "b"}
                ]
            },
            "bans": {
                "warn": [{"id": "multiple-versions", "message": "warning 3", "package": {"name": "p", "version": "1.0"}}]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
        assert_eq!(receipt.verdict.counts.errors, 0);
        assert_eq!(receipt.verdict.counts.warnings, 3);
    }

    #[test]
    fn test_verdict_calculation_with_errors_and_warnings_mixed() {
        let json = r#"{
            "licenses": {
                "deny": [{"id": "unlicensed", "message": "error 1", "license": "a"}],
                "warn": [{"id": "missing-license", "message": "warning 1", "license": "b"}]
            },
            "bans": {
                "deny": [{"id": "circular", "message": "error 2", "package": {"name": "p", "version": "1.0"}}],
                "warn": [{"id": "multiple-versions", "message": "warning 2", "package": {"name": "q", "version": "2.0"}}]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.errors, 2);
        assert_eq!(receipt.verdict.counts.warnings, 2);
    }

    #[test]
    fn test_check_id_mapping_licenses() {
        let json = r#"{
            "licenses": {
                "deny": [
                    {"id": "missing-license", "message": "msg", "license": "a"},
                    {"id": "missing", "message": "msg", "license": "b"},
                    {"id": "unlicensed", "message": "msg", "license": "c"},
                    {"id": "unknown-id", "message": "msg", "license": "d"}
                ]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(
            receipt.findings[0].check_id,
            Some("licenses.missing".to_string())
        );
        assert_eq!(
            receipt.findings[1].check_id,
            Some("licenses.missing".to_string())
        );
        assert_eq!(
            receipt.findings[2].check_id,
            Some("licenses.unlicensed".to_string())
        );
        assert_eq!(
            receipt.findings[3].check_id,
            Some("licenses.unlicensed".to_string())
        );
    }

    #[test]
    fn test_check_id_mapping_bans() {
        let json = r#"{
            "bans": {
                "deny": [
                    {"id": "multi-usage", "message": "msg", "package": {"name": "a", "version": "1.0"}},
                    {"id": "circular", "message": "msg", "package": {"name": "b", "version": "1.0"}},
                    {"id": "multiple-versions", "message": "msg", "package": {"name": "c", "version": "1.0"}},
                    {"id": "wildcard-dependencies", "message": "msg", "package": {"name": "d", "version": "1.0"}},
                    {"id": "all", "message": "msg", "package": {"name": "e", "version": "1.0"}},
                    {"id": "allow-warnings", "message": "msg", "package": {"name": "f", "version": "1.0"}},
                    {"id": "deny-warnings", "message": "msg", "package": {"name": "g", "version": "1.0"}},
                    {"id": "some-other-id", "message": "msg", "package": {"name": "h", "version": "1.0"}}
                ]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(
            receipt.findings[0].check_id,
            Some("bans.multi-usage".to_string())
        );
        assert_eq!(
            receipt.findings[1].check_id,
            Some("bans.circular".to_string())
        );
        assert_eq!(
            receipt.findings[2].check_id,
            Some("bans.multiple-versions".to_string())
        );
        assert_eq!(
            receipt.findings[3].check_id,
            Some("bans.wildcard-dependencies".to_string())
        );
        assert_eq!(receipt.findings[4].check_id, Some("bans.all".to_string()));
        assert_eq!(
            receipt.findings[5].check_id,
            Some("bans.allow-warnings".to_string())
        );
        assert_eq!(
            receipt.findings[6].check_id,
            Some("bans.deny-warnings".to_string())
        );
        assert_eq!(
            receipt.findings[7].check_id,
            Some("bans.some-other-id".to_string())
        );
    }

    #[test]
    fn test_check_id_mapping_advisories() {
        let json = r#"{
            "advisories": {
                "deny": [
                    {"id": "RUSTSEC-0001-0001", "message": "vulnerability found", "advisory_id": "RUSTSEC-0001-0001", "package": {"name": "a", "version": "1.0"}},
                    {"id": null, "message": "vulnerability found", "advisory_id": "RUSTSEC-0002-0002", "package": {"name": "b", "version": "1.0"}}
                ],
                "warn": [
                    {"id": "RUSTSEC-0003-0003", "message": "advisory warning", "advisory_id": "RUSTSEC-0003-0003", "package": {"name": "c", "version": "1.0"}},
                    {"id": null, "message": "advisory warning", "advisory_id": "RUSTSEC-0004-0004", "package": {"name": "d", "version": "1.0"}}
                ]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(
            receipt.findings[0].check_id,
            Some("advisories.RUSTSEC-0001-0001".to_string())
        );
        assert_eq!(
            receipt.findings[1].check_id,
            Some("advisories.vulnerability".to_string())
        );
        assert_eq!(
            receipt.findings[2].check_id,
            Some("advisories.RUSTSEC-0003-0003".to_string())
        );
        assert_eq!(
            receipt.findings[3].check_id,
            Some("advisories.warning".to_string())
        );
    }

    #[test]
    fn test_edge_case_empty_arrays() {
        let json = r#"{
            "licenses": {"deny": [], "warn": []},
            "bans": {"deny": [], "warn": []},
            "advisories": {"deny": [], "warn": []},
            "sources": {"deny": [], "warn": []}
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 0);
        assert_eq!(receipt.verdict.status, VerdictStatus::Pass);
    }

    #[test]
    fn test_edge_case_null_values_in_optional_fields() {
        let json = r#"{
            "licenses": {
                "deny": [
                    {"id": null, "message": "error", "license": null, "package": null}
                ]
            },
            "bans": {
                "deny": [
                    {"id": null, "message": "error", "package": null, "features": null}
                ]
            },
            "advisories": {
                "deny": [
                    {"id": null, "message": "error", "advisory_id": null, "package": null}
                ]
            },
            "sources": {
                "deny": [
                    {"id": null, "message": "error", "source": null}
                ]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 4);
        for finding in &receipt.findings {
            assert!(finding.message.is_some());
        }
    }

    #[test]
    fn test_edge_case_finding_without_location() {
        let json = r#"{
            "advisories": {
                "deny": [
                    {"id": "RUSTSEC-0001-0001", "message": "vulnerability in serde", "advisory_id": "RUSTSEC-0001-0001", "package": {"name": "serde", "version": "1.0.0"}}
                ]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert!(
            receipt.findings[0].location.is_none(),
            "advisories should have no location"
        );
    }

    #[test]
    fn test_bans_check_id_with_null_id() {
        let json = r#"{
            "bans": {
                "deny": [
                    {"id": null, "message": "error", "package": {"name": "a", "version": "1.0"}}
                ]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(
            receipt.findings[0].check_id,
            Some("bans.unknown".to_string())
        );
    }

    #[test]
    fn test_sources_check_id_mapping() {
        let json = r#"{
            "sources": {
                "deny": [{"id": "untrusted-source", "message": "untrusted", "source": "https://evil.com"}],
                "warn": [{"id": "untrusted-source", "message": "untrusted warn", "source": "https://warn.com"}]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 2);
        assert_eq!(
            receipt.findings[0].check_id,
            Some("sources.untrusted".to_string())
        );
        assert_eq!(receipt.findings[0].severity, Severity::Error);
        assert_eq!(
            receipt.findings[1].check_id,
            Some("sources.untrusted".to_string())
        );
        assert_eq!(receipt.findings[1].severity, Severity::Warn);
    }

    #[test]
    fn test_license_warning_severity_mapping() {
        let json = r#"{
            "licenses": {
                "warn": [
                    {"id": "missing-license", "message": "license file not found", "license": "my-crate"}
                ]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        assert_eq!(receipt.findings.len(), 1);
        assert_eq!(receipt.findings[0].severity, Severity::Warn);
        assert_eq!(receipt.verdict.status, VerdictStatus::Warn);
    }

    #[test]
    fn test_mixed_sections_all_have_correct_severity() {
        let json = r#"{
            "licenses": {
                "deny": [{"id": "unlicensed", "message": "error", "license": "a"}],
                "warn": [{"id": "missing-license", "message": "warn", "license": "b"}]
            },
            "bans": {
                "deny": [{"id": "circular", "message": "error", "package": {"name": "a", "version": "1.0"}}],
                "warn": [{"id": "multiple-versions", "message": "warn", "package": {"name": "b", "version": "1.0"}}]
            },
            "advisories": {
                "deny": [{"id": "RUSTSEC-0001", "message": "error", "advisory_id": "RUSTSEC-0001", "package": {"name": "a", "version": "1.0"}}],
                "warn": [{"id": "RUSTSEC-0002", "message": "warn", "advisory_id": "RUSTSEC-0002", "package": {"name": "b", "version": "1.0"}}]
            },
            "sources": {
                "deny": [{"id": "untrusted", "message": "error", "source": "a"}],
                "warn": [{"id": "untrusted", "message": "warn", "source": "b"}]
            }
        }"#;

        let report: CargoDenyReport = serde_json::from_str(json).unwrap();
        let receipt = convert_report(report).unwrap();

        let errors: Vec<_> = receipt
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .collect();
        let warnings: Vec<_> = receipt
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Warn)
            .collect();

        assert_eq!(errors.len(), 4);
        assert_eq!(warnings.len(), 4);
        assert_eq!(receipt.verdict.status, VerdictStatus::Fail);
        assert_eq!(receipt.verdict.counts.errors, 4);
        assert_eq!(receipt.verdict.counts.warnings, 4);
    }
}
