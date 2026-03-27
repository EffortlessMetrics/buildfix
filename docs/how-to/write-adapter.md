# How to Write an Adapter

This guide walks you through creating a new buildfix adapter from scratch. By the end, you'll have a working adapter that transforms your sensor's output into buildfix receipts.

## 1. Introduction

### What is an Adapter?

An adapter is a Rust crate that transforms a sensor tool's native output format into buildfix's standardized `ReceiptEnvelope` format. Adapters are the intake layer of buildfix, responsible for:

- **Parsing** sensor output (typically JSON)
- **Normalizing** findings to a common schema
- **Mapping** severities and check IDs
- **Building** receipts that fixers can consume

### Adapter Architecture Overview

The buildfix adapter system follows a hexagonal architecture pattern:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Sensor Tools                              │
│  (cargo-deny, clippy, machete, rustfmt, custom tools, etc.)     │
└──────────────────────────┬──────────────────────────────────────┘
                           │ Native output (JSON, JSONL, etc.)
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Adapter Layer                                │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │ Adapter Trait   │  │ AdapterMetadata │  │ ReceiptBuilder  │ │
│  │ - sensor_id()   │  │ - name()        │  │ - fluent API    │ │
│  │ - load()        │  │ - version()     │  │ - defaults      │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└──────────────────────────┬──────────────────────────────────────┘
                           │ ReceiptEnvelope
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Domain Layer                                 │
│              (Planner → Fixers → Edit Engine)                    │
└─────────────────────────────────────────────────────────────────┘
```

**Key Components:**

| Component | Responsibility | Location |
|-----------|---------------|----------|
| [`Adapter`] trait | Load and parse sensor output | `buildfix-adapter-sdk/src/lib.rs` |
| [`AdapterMetadata`] trait | Self-description and versioning | `buildfix-adapter-sdk/src/lib.rs` |
| [`ReceiptBuilder`] | Construct receipt envelopes | `buildfix-adapter-sdk/src/receipt_builder.rs` |
| [`AdapterTestHarness`] | Test validation utilities | `buildfix-adapter-sdk/src/harness.rs` |
| [`ReceiptEnvelope`] | Standardized receipt format | `buildfix-types/src/receipt.rs` |

**Data Flow:**

1. Sensor tool runs and produces output (e.g., `artifacts/cargo-deny/report.json`)
2. Adapter's `load()` method reads and parses the file
3. Input types (tool-specific) are converted to output types (buildfix standard)
4. `ReceiptBuilder` constructs the final `ReceiptEnvelope`
5. Domain planner routes findings to appropriate fixers

### When Do You Need to Write One?

You need to write an adapter when:

- You want buildfix to consume output from a sensor tool that isn't already supported
- You have a custom or internal tool that produces lint/analysis output
- You want to transform SARIF or other generic formats into buildfix-specific receipts

### Prerequisites

Before writing an adapter, you should have:

- **Rust knowledge**: Comfortable with structs, traits, and serde
- **Sensor understanding**: Familiarity with your tool's output format
- **buildfix basics**: Understanding of receipts and findings (see [`buildfix-types/src/receipt.rs`](../../buildfix-types/src/receipt.rs))

## 2. Quick Start

The fastest way to create an adapter:

```bash
# 1. Copy the template
cp -r buildfix-receipts-template buildfix-receipts-mytool

# 2. Update Cargo.toml with your tool's name
# 3. Rename the adapter struct in src/lib.rs
# 4. Run tests
cargo test -p buildfix-receipts-mytool
```

## 3. Step-by-Step Guide

### Step 1: Set Up the Crate

Copy the template and configure it for your tool:

```bash
cp -r buildfix-receipts-template buildfix-receipts-mytool
```

Update [`Cargo.toml`](../../buildfix-receipts-template/Cargo.toml):

```toml
[package]
name = "buildfix-receipts-mytool"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Adapter for mytool sensor output"
repository.workspace = true

[dependencies]
anyhow.workspace = true
camino.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
buildfix-types = { version = "0.3.0", path = "../buildfix-types" }
buildfix-adapter-sdk = { version = "0.3.0", path = "../buildfix-adapter-sdk" }

[dev-dependencies]
pretty_assertions.workspace = true
tempfile.workspace = true
```

Add to the workspace in the root [`Cargo.toml`](../../Cargo.toml):

```toml
members = [
    # ... existing members ...
    "buildfix-receipts-mytool",
]
```

### Step 2: Understand Your Sensor's Output

Before writing code, analyze your sensor's output format:

1. **Run your sensor** and capture sample output
2. **Identify the JSON structure** (or whatever format it uses)
3. **Note the fields** you need to map: findings, severities, locations, messages
4. **Save a sample** as `tests/fixtures/report.json`

Example analysis for a hypothetical tool:

```json
{
  "version": "2.0",
  "issues": [
    {
      "rule": "MAGIC001",
      "level": "error",
      "description": "Magic number detected",
      "file": "src/main.rs",
      "line": 42
    }
  ]
}
```

### Step 3: Define the Input Schema

Create Rust structs that match your sensor's JSON output:

```rust
use serde::Deserialize;

/// Root structure of the sensor output
#[derive(Debug, Deserialize)]
struct MyToolReport {
    #[serde(default)]
    version: String,
    
    #[serde(default)]
    issues: Vec<MyToolIssue>,
}

/// A single issue from the sensor
#[derive(Debug, Deserialize, Clone)]
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
```

**Tips for handling optional fields:**

- Use `#[serde(default)]` for fields that may be missing
- Use `Option<T>` for nullable fields
- Use `#[serde(rename = "...")]` to map JSON field names to Rust conventions

### Step 4: Implement the Adapter Trait

Implement the [`Adapter`](../../buildfix-adapter-sdk/src/lib.rs) trait:

```rust
use buildfix_adapter_sdk::{Adapter, AdapterError};
use buildfix_types::receipt::ReceiptEnvelope;
use std::path::Path;

pub struct MyToolAdapter {
    sensor_id: &'static str,
}

impl MyToolAdapter {
    pub fn new() -> Self {
        Self {
            sensor_id: "my-tool",
        }
    }
}

impl Default for MyToolAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for MyToolAdapter {
    fn sensor_id(&self) -> &str {
        self.sensor_id
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        // 1. Read the file
        let content = std::fs::read_to_string(path)
            .map_err(AdapterError::Io)?;
        
        // 2. Parse JSON into your types
        let report: MyToolReport = serde_json::from_str(&content)
            .map_err(AdapterError::Json)?;
        
        // 3. Convert to ReceiptEnvelope
        convert_report(report)
    }
}
```

### Step 5: Implement AdapterMetadata

Implement the [`AdapterMetadata`](../../buildfix-adapter-sdk/src/lib.rs) trait for self-description:

```rust
use buildfix_adapter_sdk::AdapterMetadata;

impl AdapterMetadata for MyToolAdapter {
    /// Unique identifier matching the sensor tool name
    fn name(&self) -> &str {
        "my-tool"
    }

    /// Adapter version from Cargo.toml
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    /// Schema versions this adapter can parse
    fn supported_schemas(&self) -> &[&str] {
        &["my-tool.report.v1"]
    }
}
```

### Step 6: Map Check IDs

Check IDs follow the naming convention: `sensor.category.specific`

```rust
fn map_rule_to_check_id(rule: &str) -> String {
    // Examples:
    // - "my-tool.style.magic-number"
    // - "my-tool.security.hardcoded-secret"
    // - "my-tool.performance.unnecessary-clone"
    
    match rule {
        "MAGIC001" => "my-tool.style.magic-number".to_string(),
        "SECRET001" => "my-tool.security.hardcoded-secret".to_string(),
        "CLONE001" => "my-tool.performance.unnecessary-clone".to_string(),
        _ => format!("my-tool.unknown.{}", rule.to_lowercase()),
    }
}
```

**Naming rules:**

- All lowercase
- At least 2 dots (3+ segments)
- Each segment is alphanumeric with hyphens/underscores allowed
- First segment should match the sensor ID

See [Check ID Naming Convention](#4-check-id-naming-convention) for more details.

### Step 7: Map Severities

Map your sensor's severity levels to buildfix's [`Severity`](../../buildfix-types/src/receipt.rs) enum:

```rust
use buildfix_types::receipt::Severity;

fn map_severity(level: &str) -> Severity {
    match level.to_lowercase().as_str() {
        "error" | "fatal" | "critical" => Severity::Error,
        "warning" | "warn" => Severity::Warn,
        "info" | "note" | "suggestion" => Severity::Info,
        _ => Severity::Info, // Default to info for unknown levels
    }
}
```

The buildfix `Severity` enum has three levels:

- **Error**: Blocking issues that should fail the build
- **Warn**: Non-blocking issues that should be reviewed
- **Info**: Informational findings, suggestions

### Step 8: Build the Receipt

Use the [`ReceiptBuilder`](../../buildfix-adapter-sdk/src/receipt_builder.rs) pattern to construct receipts:

```rust
use buildfix_adapter_sdk::ReceiptBuilder;
use buildfix_types::receipt::{Finding, Location, VerdictStatus};
use camino::Utf8PathBuf;

fn convert_report(report: MyToolReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut error_count = 0u64;
    let mut warn_count = 0u64;

    for issue in &report.issues {
        let severity = map_severity(&issue.level);
        
        match severity {
            Severity::Error => error_count += 1,
            Severity::Warn => warn_count += 1,
            _ => {}
        }

        let location = Location {
            path: Utf8PathBuf::from(&issue.file_path),
            line: issue.line,
            column: issue.column,
        };

        let check_id = map_rule_to_check_id(&issue.rule);

        findings.push(Finding {
            severity,
            check_id: Some(check_id),
            code: Some(issue.rule.clone()),
            message: Some(issue.description.clone()),
            location: Some(location),
            fingerprint: None,
            data: None,
        });
    }

    // Determine overall status
    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    // Build the receipt
    let receipt = ReceiptBuilder::new("my-tool")
        .with_schema("my-tool.report.v1")
        .with_tool_version("1.0.0")
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, warn_count)
        .with_findings(findings)
        .build();

    Ok(receipt)
}
```

### Step 9: Write Tests

Create comprehensive tests using the [`AdapterTestHarness`](../../buildfix-adapter-sdk/src/harness.rs):

**Fixture file** (`tests/fixtures/report.json`):

```json
{
  "version": "1.0",
  "issues": [
    {
      "rule": "MAGIC001",
      "level": "error",
      "description": "Magic number detected: 42",
      "file": "src/main.rs",
      "line": 10
    },
    {
      "rule": "SECRET001",
      "level": "warning",
      "description": "Hardcoded API key",
      "file": "src/config.rs",
      "line": 5
    }
  ]
}
```

**Test file** (`tests/adapter_test.rs`):

```rust
use buildfix_adapter_sdk::{Adapter, AdapterMetadata, AdapterTestHarness};
use buildfix_types::receipt::Severity;
use my_tool_adapter::MyToolAdapter;

#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(MyToolAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");
    
    assert!(!receipt.schema.is_empty());
    assert_eq!(receipt.tool.name, "my-tool");
}

#[test]
fn test_metadata() {
    let harness = AdapterTestHarness::new(MyToolAdapter::new());
    harness.validate_metadata(harness.adapter())
        .expect("metadata should be valid");
}

#[test]
fn test_check_id_format() {
    let harness = AdapterTestHarness::new(MyToolAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");
    
    harness.validate_check_id_format(&receipt)
        .expect("all check IDs should follow the naming convention");
}

#[test]
fn test_location_paths() {
    let harness = AdapterTestHarness::new(MyToolAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");
    
    harness.validate_location_paths(&receipt)
        .expect("all paths should be valid");
}

#[test]
fn test_finding_severities() {
    let harness = AdapterTestHarness::new(MyToolAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");
    
    // Verify expected counts
    harness.assert_finding_count(&receipt, 1, Some(Severity::Error))
        .expect("should have 1 error");
    harness.assert_finding_count(&receipt, 1, Some(Severity::Warn))
        .expect("should have 1 warning");
}
```

Run tests:

```bash
cargo test -p buildfix-receipts-mytool
```

### Step 10: Document

Update documentation in your adapter crate:

**CLAUDE.md** (for AI assistant guidance):

```markdown
# buildfix-receipts-mytool

Adapter for my-tool sensor output.

## Check ID Mapping

| Tool Rule | buildfix Check ID |
|-----------|-------------------|
| MAGIC001 | my-tool.style.magic-number |
| SECRET001 | my-tool.security.hardcoded-secret |

## Input Format

Expects JSON output from `my-tool --output json`.
```

**README.md**:

```markdown
# buildfix-receipts-mytool

Adapter for [my-tool](https://example.com/my-tool) sensor output.

## Usage

This adapter is used internally by buildfix. To generate receipts:

```bash
my-tool --output json > artifacts/my-tool/report.json
cargo run -p buildfix -- plan
```

## Supported Schema Versions

- `my-tool.report.v1`
```

## 4. Check ID Naming Convention

Check IDs enable buildfix to route findings to appropriate fixers. Follow these conventions:

### Format

```
<sensor>.<category>.<specific>
```

- **sensor**: Matches the adapter's sensor ID (e.g., `cargo-deny`, `machete`)
- **category**: Broad classification (e.g., `ban`, `license`, `security`)
- **specific**: Specific issue type (e.g., `multiple-versions`, `unused-dep`)

### Examples

| Sensor | Check ID | Description |
|--------|----------|-------------|
| cargo-deny | `cargo-deny.ban.multiple-versions` | Duplicate dependency versions |
| cargo-deny | `cargo-deny.license.unlicensed` | Missing license |
| machete | `machete.unused_dependency` | Unused dependency |
| clippy | `clippy.style.unnecessary_clone` | Unnecessary `.clone()` call |

### Validation Rules

The harness validates check IDs with these rules:

1. Must be lowercase
2. Must contain at least 2 dots (3+ segments)
3. Each segment must be non-empty
4. Each segment allows: alphanumeric, hyphens, underscores

Validate with:

```rust
harness.validate_check_id_format(&receipt)
    .expect("check IDs should be valid");
```

## 5. Testing Your Adapter

### Using AdapterTestHarness

The [`AdapterTestHarness`](../../buildfix-adapter-sdk/src/harness.rs) provides validation methods:

```rust
use buildfix_adapter_sdk::AdapterTestHarness;

let harness = AdapterTestHarness::new(MyToolAdapter::new());

// Load and validate a fixture
let receipt = harness.validate_receipt_fixture("tests/fixtures/report.json")?;

// Validate metadata
harness.validate_metadata(&adapter)?;

// Validate check ID format
harness.validate_check_id_format(&receipt)?;

// Validate location paths
harness.validate_location_paths(&receipt)?;

// Validate finding fields
harness.validate_finding_fields(&receipt)?;

// Assert finding counts
harness.assert_finding_count(&receipt, 5, None)?; // Total
harness.assert_finding_count(&receipt, 2, Some(Severity::Error))?; // Errors only

// Check for specific check ID
harness.assert_has_check_id(&receipt, "my-tool.style.magic-number")?;

// Extract all check IDs
let check_ids = harness.extract_check_ids(&receipt);
```

### Golden Tests

For regression testing, use golden tests to compare output against expected:

```rust
#[test]
fn test_golden() {
    let harness = AdapterTestHarness::new(MyToolAdapter::new());
    let expected = /* ... expected receipt ... */;
    
    harness.golden_test("tests/fixtures/report.json", &expected)
        .expect("output should match golden file");
}
```

### Test Coverage Checklist

- [ ] Adapter loads valid fixture without errors
- [ ] Sensor ID matches expected value
- [ ] Metadata validation passes
- [ ] All check IDs follow naming convention
- [ ] All location paths are normalized
- [ ] Severity mapping is correct
- [ ] Finding counts match expected
- [ ] Verdict status reflects findings
- [ ] Empty input produces valid receipt with no findings

## 6. Common Patterns

### Error Handling

Use [`AdapterError`](../../buildfix-adapter-sdk/src/lib.rs) variants:

```rust
use buildfix_adapter_sdk::AdapterError;

fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
    // IO errors
    let content = std::fs::read_to_string(path)
        .map_err(AdapterError::Io)?;
    
    // JSON parsing errors
    let report: MyReport = serde_json::from_str(&content)
        .map_err(AdapterError::Json)?;
    
    // Custom validation errors
    if report.version.is_empty() {
        return Err(AdapterError::InvalidFormat(
            "report version is required".to_string()
        ));
    }
    
    // Missing required fields
    let field = report.required_field
        .ok_or_else(|| AdapterError::MissingField("required_field".to_string()))?;
    
    // ... rest of conversion
}
```

### Path Normalization

Ensure paths are repo-relative with forward slashes:

```rust
use camino::Utf8PathBuf;

fn normalize_path(path: &str) -> String {
    // Remove leading ./
    let path = path.strip_prefix("./").unwrap_or(path);
    
    // Convert backslashes to forward slashes
    path.replace('\\', "/")
}

let location = Location {
    path: Utf8PathBuf::from(normalize_path(&issue.file_path)),
    line: issue.line,
    column: issue.column,
};
```

### Optional Fields

Handle optional fields gracefully:

```rust
#[derive(Debug, Deserialize)]
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

fn default_level() -> String {
    "info".to_string()
}
```

### Multiple Check IDs

When a sensor produces multiple types of findings:

```rust
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
```

## 7. Troubleshooting

### Common Errors

**"JSON parse error"**

- Verify your input structs match the JSON structure
- Check for typos in `#[serde(rename = "...")]` attributes
- Ensure optional fields use `Option<T>` or `#[serde(default)]`

**"Invalid check ID format"**

- Check IDs must be lowercase
- Must have at least 2 dots (3+ segments)
- Use `validate_check_id_format()` to debug

**"Location path validation failed"**

- Paths must use forward slashes
- Paths must be relative (no leading `/`)
- No `..` parent directory references

**"Empty receipt"**

- Check that your fixture file has actual findings
- Verify the JSON parsing is working (add debug prints)
- Ensure `convert_report` is returning findings

### Debugging Tips

1. **Add debug output** during development:

```rust
fn convert_report(report: MyReport) -> Result<ReceiptEnvelope, AdapterError> {
    eprintln!("Parsed report: {:?}", report);
    // ... rest of conversion
}
```

2. **Test parsing separately**:

```rust
#[test]
fn test_parse_json() {
    let json = include_str!("../tests/fixtures/report.json");
    let report: MyReport = serde_json::from_str(json).unwrap();
    eprintln!("{:#?}", report);
}
```

3. **Use serde's `json!` macro** for inline test data:

```rust
#[test]
fn test_severity_mapping() {
    let json = json!({"level": "error", "message": "test"});
    let finding: MyFinding = serde_json::from_value(json).unwrap();
    assert_eq!(map_severity(&finding.level), Severity::Error);
}
```

## 8. Example: Full Implementation

For a complete reference implementation, see:

- **Template adapter**: [`buildfix-receipts-template/`](../../buildfix-receipts-template/)
- **Cargo-deny adapter**: [`buildfix-receipts-cargo-deny/`](../../buildfix-receipts-cargo-deny/)
- **Cargo-machete adapter**: [`buildfix-receipts-cargo-machete/`](../../buildfix-receipts-cargo-machete/)

### Template Structure

```
buildfix-receipts-template/
├── Cargo.toml              # Package configuration
├── CLAUDE.md               # AI assistant guidance
├── README.md               # User documentation
├── src/
│   └── lib.rs              # Adapter implementation
└── tests/
    ├── adapter_test.rs     # Integration tests
    └── fixtures/
        └── report.json     # Sample input
```

### Key Files to Reference

| File | Purpose |
|------|---------|
| [`buildfix-adapter-sdk/src/lib.rs`](../../buildfix-adapter-sdk/src/lib.rs) | Core traits and types |
| [`buildfix-adapter-sdk/src/harness.rs`](../../buildfix-adapter-sdk/src/harness.rs) | Test harness |
| [`buildfix-adapter-sdk/src/receipt_builder.rs`](../../buildfix-adapter-sdk/src/receipt_builder.rs) | Builder pattern |
| [`buildfix-types/src/receipt.rs`](../../buildfix-types/src/receipt.rs) | Receipt types |

## 9. Receipt Format Reference

### ReceiptEnvelope Structure

The `ReceiptEnvelope` is the top-level container for all receipt data:

```rust
pub struct ReceiptEnvelope {
    /// Schema identifier, e.g., "cargo-deny.report.v1"
    pub schema: String,
    
    /// Tool information
    pub tool: ToolInfo,
    
    /// Run metadata (timestamps, git SHA)
    pub run: RunInfo,
    
    /// Overall verdict and counts
    pub verdict: Verdict,
    
    /// Individual findings
    pub findings: Vec<Finding>,
    
    /// Optional capabilities block
    pub capabilities: Option<ReceiptCapabilities>,
    
    /// Optional tool-specific data
    pub data: Option<serde_json::Value>,
}
```

### Required Fields

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `schema` | `String` | Schema version identifier | `"cargo-deny.report.v1"` |
| `tool.name` | `String` | Tool/sensor name | `"cargo-deny"` |
| `verdict.status` | `VerdictStatus` | Overall pass/fail/warn | `Fail` |
| `verdict.counts.findings` | `u64` | Total finding count | `5` |

### Finding Structure

Each finding represents a single issue detected by the sensor:

```rust
pub struct Finding {
    /// Severity level (Error, Warn, Info)
    pub severity: Severity,
    
    /// Check ID following naming convention
    pub check_id: Option<String>,
    
    /// Original rule/code from the tool
    pub code: Option<String>,
    
    /// Human-readable message
    pub message: Option<String>,
    
    /// File location
    pub location: Option<Location>,
    
    /// Stable key for deduplication
    pub fingerprint: Option<String>,
    
    /// Tool-specific payload
    pub data: Option<serde_json::Value>,
}
```

### Location Structure

File locations should be normalized to repo-relative paths:

```rust
pub struct Location {
    /// Relative path with forward slashes
    pub path: Utf8PathBuf,
    
    /// 1-based line number
    pub line: Option<u64>,
    
    /// 1-based column number
    pub column: Option<u64>,
}
```

### Example Receipt JSON

```json
{
  "schema": "my-tool.report.v1",
  "tool": {
    "name": "my-tool",
    "version": "1.0.0"
  },
  "run": {
    "started_at": "2024-01-15T10:30:00Z",
    "ended_at": "2024-01-15T10:30:05Z",
    "git_head_sha": "abc123def456"
  },
  "verdict": {
    "status": "fail",
    "counts": {
      "findings": 3,
      "errors": 1,
      "warnings": 2
    },
    "reasons": ["Found 1 error and 2 warnings"]
  },
  "findings": [
    {
      "severity": "error",
      "check_id": "my-tool.security.hardcoded-secret",
      "code": "SECRET001",
      "message": "Hardcoded API key detected",
      "location": {
        "path": "src/config.rs",
        "line": 42,
        "column": 10
      }
    },
    {
      "severity": "warn",
      "check_id": "my-tool.style.magic-number",
      "code": "MAGIC001",
      "message": "Magic number without constant",
      "location": {
        "path": "src/math.rs",
        "line": 15
      }
    }
  ],
  "capabilities": {
    "check_ids": ["my-tool.security.hardcoded-secret", "my-tool.style.magic-number"],
    "scopes": ["workspace"],
    "partial": false
  }
}
```

### Capabilities Block

The `capabilities` block implements the "No Green By Omission" pattern:

```rust
pub struct ReceiptCapabilities {
    /// All check IDs this sensor can emit
    pub check_ids: Vec<String>,
    
    /// Scopes covered (e.g., "workspace", "crate")
    pub scopes: Vec<String>,
    
    /// True if some inputs couldn't be processed
    pub partial: bool,
    
    /// Reason for partial results
    pub reason: Option<String>,
}
```

Use capabilities when:
- The sensor might not check everything (partial runs)
- You want to track what the sensor *can* detect
- CI needs to verify complete coverage

## 10. Registering Your Adapter

### Adding to the Adapter Catalog

To make your adapter available to buildfix users:

1. **Add to workspace Cargo.toml**:

```toml
[workspace.members]
members = [
    # ... existing members ...
    "buildfix-receipts-mytool",
]
```

2. **Add to the adapter registry** (if contributing to the main repo):

In `buildfix-cli/src/adapters.rs` or equivalent:

```rust
use buildfix_receipts_mytool::MyToolAdapter;

pub fn register_adapters(registry: &mut AdapterRegistry) {
    registry.register("my-tool", Box::new(MyToolAdapter::new()));
}
```

3. **Update documentation**:

Add your adapter to the list of supported sensors in:
- Main `README.md`
- `docs/supported-sensors.md` (if exists)

### Adapter Discovery

buildfix discovers adapters by:

1. Scanning `artifacts/<sensor_id>/report.json`
2. Matching `sensor_id` to registered adapters
3. Loading and parsing with the appropriate adapter

The `sensor_id` must match the directory name in `artifacts/`:

```
artifacts/
├── cargo-deny/
│   └── report.json     # Loaded by CargoDenyAdapter (sensor_id: "cargo-deny")
├── clippy/
│   └── report.json     # Loaded by ClippyAdapter (sensor_id: "clippy")
└── my-tool/
    └── report.json     # Loaded by MyToolAdapter (sensor_id: "my-tool")
```

## 11. Publishing to crates.io

### Pre-Publish Checklist

- [ ] All tests pass: `cargo test -p buildfix-receipts-mytool`
- [ ] Documentation is complete: `cargo doc -p buildfix-receipts-mytool`
- [ ] README.md describes the adapter and usage
- [ ] Cargo.toml has correct metadata
- [ ] Version follows semver
- [ ] License is specified

### Cargo.toml Metadata

Ensure your `Cargo.toml` has complete metadata:

```toml
[package]
name = "buildfix-receipts-mytool"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Adapter for my-tool sensor output"
repository.workspace = true
homepage.workspace = true
readme = "README.md"
keywords = ["buildfix", "adapter", "my-tool", "linting"]
categories = ["development-tools", "development-tools::build-utils"]

[dependencies]
anyhow.workspace = true
camino.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
buildfix-types = { version = "0.3.0", path = "../buildfix-types" }
buildfix-adapter-sdk = { version = "0.3.0", path = "../buildfix-adapter-sdk" }

[dev-dependencies]
pretty_assertions.workspace = true
tempfile.workspace = true
```

### Publishing Steps

```bash
# 1. Dry run to check for issues
cargo publish -p buildfix-receipts-mytool --dry-run

# 2. Publish to crates.io
cargo publish -p buildfix-receipts-mytool

# 3. Verify publication
cargo search buildfix-receipts-mytool
```

### Version Compatibility

Follow semantic versioning:

- **Major (1.x.x → 2.0.0)**: Breaking changes to public API
- **Minor (0.1.0 → 0.2.0)**: New features, backward compatible
- **Patch (0.1.0 → 0.1.1)**: Bug fixes, backward compatible

For adapters specifically:
- Bump minor when adding new check IDs
- Bump patch when fixing parsing bugs
- Bump major if `ReceiptEnvelope` output structure changes

## 12. Best Practices Summary

### Code Quality

✅ **Do:**
- Use `#[serde(default)]` for optional fields
- Implement both `Adapter` and `AdapterMetadata` traits
- Normalize paths to forward slashes
- Map all severity levels appropriately
- Write comprehensive tests

❌ **Don't:**
- Panic on malformed input (return `AdapterError` instead)
- Use absolute paths in locations
- Skip the test harness validation
- Ignore edge cases (empty reports, missing fields)

### Error Handling

```rust
// Good: Descriptive errors
Err(AdapterError::InvalidFormat(
    format!("Expected 'version' field, got: {:?}", report.version)
))

// Bad: Generic errors
Err(AdapterError::InvalidFormat("Invalid report".to_string()))
```

### Check ID Naming

```rust
// Good: Follows convention
"my-tool.security.hardcoded-secret"
"my-tool.style.magic-number"
"my-tool.performance.unnecessary-clone"

// Bad: Doesn't follow convention
"HardcodedSecret"          // Missing dots, not lowercase
"my-tool.secret"           // Only 2 segments
"MY_TOOL.SECRET.KEY"       // Uppercase
```

### Testing

```rust
// Good: Comprehensive test with harness
#[test]
fn test_adapter_loads_receipt() {
    let harness = AdapterTestHarness::new(MyToolAdapter::new());
    let receipt = harness
        .validate_receipt_fixture("tests/fixtures/report.json")
        .expect("receipt should load");
    
    harness.validate_check_id_format(&receipt).expect("check IDs valid");
    harness.validate_location_paths(&receipt).expect("paths valid");
}

// Bad: Minimal test without validation
#[test]
fn test_loads() {
    let adapter = MyToolAdapter::new();
    adapter.load(Path::new("tests/fixtures/report.json")).unwrap();
}
```

---

## Next Steps

After creating your adapter:

1. **Test thoroughly** with real sensor output
2. **Add to the adapter catalog** in buildfix-cli
3. **Publish to crates.io** for community use
4. **Create a PR** with your implementation
5. **Document any edge cases** discovered during testing

For questions or issues, consult the existing adapters or open a discussion on GitHub.

## Appendix A: Complete Adapter Example

Here's a complete, minimal adapter implementation:

```rust
//! Adapter for my-tool sensor output.

use buildfix_adapter_sdk::{Adapter, AdapterError, AdapterMetadata, ReceiptBuilder};
use buildfix_types::receipt::{Finding, Location, ReceiptEnvelope, Severity, VerdictStatus};
use camino::Utf8PathBuf;
use serde::Deserialize;
use std::path::Path;

/// Adapter for my-tool.
pub struct MyToolAdapter;

impl MyToolAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MyToolAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for MyToolAdapter {
    fn sensor_id(&self) -> &str {
        "my-tool"
    }

    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError> {
        let content = std::fs::read_to_string(path).map_err(AdapterError::Io)?;
        let report: MyToolReport = serde_json::from_str(&content).map_err(AdapterError::Json)?;
        convert_report(report)
    }
}

impl AdapterMetadata for MyToolAdapter {
    fn name(&self) -> &str {
        "my-tool"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn supported_schemas(&self) -> &[&str] {
        &["my-tool.report.v1"]
    }
}

// Input types matching tool's JSON structure
#[derive(Debug, Deserialize)]
struct MyToolReport {
    #[serde(default)]
    version: String,
    #[serde(default)]
    issues: Vec<MyToolIssue>,
}

#[derive(Debug, Deserialize, Clone)]
struct MyToolIssue {
    rule: String,
    level: String,
    message: String,
    file: String,
    #[serde(default)]
    line: Option<u64>,
}

// Conversion logic
fn convert_report(report: MyToolReport) -> Result<ReceiptEnvelope, AdapterError> {
    let mut findings = Vec::new();
    let mut error_count = 0u64;
    let mut warn_count = 0u64;

    for issue in &report.issues {
        let severity = map_severity(&issue.level);
        
        match severity {
            Severity::Error => error_count += 1,
            Severity::Warn => warn_count += 1,
            Severity::Info => {}
        }

        findings.push(Finding {
            severity,
            check_id: Some(format!("my-tool.code.{}", issue.rule.to_lowercase())),
            code: Some(issue.rule.clone()),
            message: Some(issue.message.clone()),
            location: Some(Location {
                path: Utf8PathBuf::from(issue.file.replace('\\', "/")),
                line: issue.line,
                column: None,
            }),
            fingerprint: None,
            data: None,
            ..Default::default()
        });
    }

    let status = if error_count > 0 {
        VerdictStatus::Fail
    } else if warn_count > 0 {
        VerdictStatus::Warn
    } else {
        VerdictStatus::Pass
    };

    let mut builder = ReceiptBuilder::new("my-tool")
        .with_schema("my-tool.report.v1")
        .with_tool_version(report.version)
        .with_status(status)
        .with_counts(findings.len() as u64, error_count, warn_count);

    for finding in findings {
        builder = builder.with_finding(finding);
    }

    Ok(builder.build())
}

fn map_severity(level: &str) -> Severity {
    match level.to_lowercase().as_str() {
        "error" | "fatal" => Severity::Error,
        "warning" | "warn" => Severity::Warn,
        _ => Severity::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buildfix_adapter_sdk::AdapterTestHarness;

    #[test]
    fn test_sensor_id() {
        let adapter = MyToolAdapter::new();
        assert_eq!(adapter.sensor_id(), "my-tool");
    }

    #[test]
    fn test_metadata() {
        let adapter = MyToolAdapter::new();
        assert_eq!(adapter.name(), "my-tool");
        assert!(!adapter.version().is_empty());
        assert!(!adapter.supported_schemas().is_empty());
    }

    #[test]
    fn test_severity_mapping() {
        assert_eq!(map_severity("error"), Severity::Error);
        assert_eq!(map_severity("warning"), Severity::Warn);
        assert_eq!(map_severity("info"), Severity::Info);
    }
}
```

## Appendix B: Test Fixture Example

`tests/fixtures/report.json`:

```json
{
  "version": "1.0.0",
  "issues": [
    {
      "rule": "MAGIC001",
      "level": "error",
      "message": "Magic number detected: 42",
      "file": "src/main.rs",
      "line": 10
    },
    {
      "rule": "STYLE001",
      "level": "warning",
      "message": "Missing documentation",
      "file": "src/lib.rs",
      "line": 5
    }
  ]
}
```

## Appendix C: Existing Adapters Reference

| Adapter Crate | Sensor Tool | Input Format |
|---------------|-------------|--------------|
| `buildfix-receipts-cargo-deny` | cargo-deny | JSON |
| `buildfix-receipts-cargo-machete` | cargo-machete | JSON |
| `buildfix-receipts-cargo-udeps` | cargo-udeps | JSON |
| `buildfix-receipts-cargo-outdated` | cargo-outdated | JSON |
| `buildfix-receipts-cargo-audit` | cargo-audit | JSON |
| `buildfix-receipts-clippy` | clippy | JSON |
| `buildfix-receipts-rustc-json` | rustc | JSONL |
| `buildfix-receipts-rustfmt` | rustfmt | JSON |
| `buildfix-receipts-sarif` | SARIF tools | SARIF JSON |
| `buildfix-receipts-depguard` | depguard | JSON |
| `buildfix-receipts-tarpaulin` | tarpaulin | JSON |

Study these adapters for real-world patterns and edge case handling.
