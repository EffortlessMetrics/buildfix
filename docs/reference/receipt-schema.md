# Receipt Schema Reference

This document describes the receipt model used by buildfix for ingesting sensor outputs. Receipts are JSON documents that follow a standardized envelope structure, enabling consistent processing across diverse sensor tools.

## Overview

buildfix uses a **receipt-driven** architecture where sensor tools produce findings in a standardized format. Each receipt is an `ReceiptEnvelope` containing:

- **Tool metadata**: Information about the sensor that produced the receipt
- **Run information**: Timing and git context for the sensor run
- **Verdict**: Overall pass/fail status with counts
- **Findings**: Individual issues detected by the sensor
- **Capabilities**: Optional metadata about what the sensor can/did check

### Design Principles

1. **Tolerance**: buildfix is tolerant when reading receipts:
   - Unknown fields are ignored
   - Optional fields may be absent
   - The director and sensors enforce stricter schema compliance

2. **Backward Compatibility**: Be conservative with breaking changes; prefer adding optional fields over changing semantics

3. **Determinism**: Receipts should produce deterministic results when processed

---

## Receipt Envelope Structure

The `ReceiptEnvelope` is the top-level container for all receipt data.

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema` | `string` | Yes | Schema identifier, e.g., `"sensor.report.v1"` |
| `tool` | [`ToolInfo`](#toolinfo) | Yes | Information about the sensor tool |
| `run` | [`RunInfo`](#runinfo) | No | Timing and git context for the run |
| `verdict` | [`Verdict`](#verdict) | No | Overall pass/fail status |
| `findings` | `array` of [`Finding`](#finding) | No | List of issues detected |
| `capabilities` | [`ReceiptCapabilities`](#receiptcapabilities) | No | What the sensor can/did check |
| `data` | `object` | No | Tool-specific payload |

### JSON Example

```json
{
  "schema": "sensor.report.v1",
  "tool": {
    "name": "cargo-deny",
    "version": "0.14.0",
    "repo": "https://github.com/EmbarkStudios/cargo-deny",
    "commit": "abc123def456"
  },
  "run": {
    "started_at": "2024-01-15T10:30:00Z",
    "ended_at": "2024-01-15T10:30:45Z",
    "git_head_sha": "def789abc012"
  },
  "verdict": {
    "status": "fail",
    "counts": {
      "findings": 5,
      "errors": 2,
      "warnings": 3
    },
    "reasons": ["license issues detected", "duplicate dependencies found"]
  },
  "findings": [
    {
      "severity": "error",
      "check_id": "licenses.unlicensed",
      "code": "unlicensed",
      "message": "no license field was found in the Cargo.toml manifest",
      "location": {
        "path": "crates/my-crate/Cargo.toml",
        "line": 10,
        "column": 1
      },
      "fingerprint": "sha256:abc123...",
      "data": {
        "package": "my-crate",
        "version": "0.1.0"
      }
    }
  ],
  "capabilities": {
    "check_ids": ["licenses.missing", "licenses.unlicensed", "bans.duplicate"],
    "scopes": ["workspace"],
    "partial": false
  }
}
```

---

## ToolInfo Structure

Describes the sensor tool that produced the receipt.

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `string` | Yes | Tool name (e.g., `"cargo-deny"`, `"clippy"`) |
| `version` | `string` | No | Tool version |
| `repo` | `string` | No | Repository URL |
| `commit` | `string` | No | Git commit SHA |

### JSON Example

```json
{
  "name": "cargo-udeps",
  "version": "0.1.45",
  "repo": "https://github.com/est31/cargo-udeps",
  "commit": "v0.1.45"
}
```

---

## RunInfo Structure

Contains timing and git context for the sensor run.

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `started_at` | `string` (ISO 8601) | No | When the sensor run started |
| `ended_at` | `string` (ISO 8601) | No | When the sensor run ended |
| `git_head_sha` | `string` | No | Git HEAD SHA at run time |

### JSON Example

```json
{
  "started_at": "2024-01-15T10:30:00.123Z",
  "ended_at": "2024-01-15T10:30:45.789Z",
  "git_head_sha": "abc123def456789"
}
```

### Usage Notes

- The `git_head_sha` is used to verify that a plan is applied to the same repository state it was generated from
- Timestamps should be in UTC with ISO 8601 format

---

## Verdict Structure

Summarizes the overall result of the sensor run.

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `status` | [`VerdictStatus`](#verdictstatus-enum) | No | Overall status (defaults to `unknown`) |
| `counts` | [`Counts`](#counts) | No | Count of findings by severity |
| `reasons` | `array` of `string` | No | Human-readable reasons for the verdict |

### VerdictStatus Enum

| Value | Description |
|-------|-------------|
| `pass` | All checks passed |
| `warn` | Warnings found but no errors |
| `fail` | Errors found |
| `unknown` | Status could not be determined (default) |

### Counts Structure

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `findings` | `integer` | No | Total number of findings |
| `errors` | `integer` | No | Number of error-level findings |
| `warnings` | `integer` | No | Number of warning-level findings |

### JSON Example

```json
{
  "status": "fail",
  "counts": {
    "findings": 10,
    "errors": 3,
    "warnings": 7
  },
  "reasons": [
    "3 license violations detected",
    "7 duplicate dependencies found"
  ]
}
```

---

## Finding Structure

Represents a single issue detected by a sensor.

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `severity` | [`Severity`](#severity-enum) | No | Issue severity (defaults to `info`) |
| `check_id` | `string` | No | Check identifier for categorization |
| `code` | `string` | No | Tool-specific error code |
| `message` | `string` | No | Human-readable description |
| `location` | [`Location`](#location-structure) | No | File location of the issue |
| `fingerprint` | `string` | No | Stable key for deduplication across runs |
| `data` | `object` | No | Tool-specific payload |
| `confidence` | `number` | No | Confidence score (0.0 to 1.0) |
| `provenance` | [`Provenance`](#provenance-structure) | No | How the finding was derived |
| `context` | [`FindingContext`](#findingcontext-structure) | No | Additional context metadata |

### Severity Enum

| Value | Description |
|-------|-------------|
| `info` | Informational finding (default) |
| `warn` | Warning-level issue |
| `error` | Error-level issue requiring attention |

### JSON Example

```json
{
  "severity": "error",
  "check_id": "deps.unused_dependency",
  "code": "unused",
  "message": "unused dependency: serde",
  "location": {
    "path": "Cargo.toml",
    "line": 15,
    "column": 1
  },
  "fingerprint": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "data": {
    "name": "serde",
    "version": "1.0.0",
    "kind": "Normal"
  },
  "confidence": 0.95,
  "provenance": {
    "method": "dead_code_analysis",
    "tools": ["cargo-udeps"],
    "agreement": false,
    "evidence_chain": [
      {
        "source": "analysis",
        "value": "no usage of serde found in src/",
        "validated": true
      }
    ]
  },
  "context": {
    "workspace": {
      "consensus_value": null,
      "consensus_count": 0,
      "total_crates": 5,
      "outliers": [],
      "outlier_crates": [],
      "all_crates_agree": true
    },
    "analysis_depth": "full"
  }
}
```

---

## Location Structure

Specifies a file location for a finding.

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | `string` | Yes | File path (repo-relative, forward slashes) |
| `line` | `integer` | No | Line number (1-based) |
| `column` | `integer` | No | Column number (1-based) |

### JSON Example

```json
{
  "path": "crates/my-crate/Cargo.toml",
  "line": 25,
  "column": 5
}
```

### Path Normalization

- Paths are repo-relative
- Always use forward slashes (`/`)
- No leading `./`

---

## ReceiptCapabilities Structure

Describes what checks a sensor can perform and whether it ran completely. This enables the **"No Green By Omission"** pattern.

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `check_ids` | `array` of `string` | No | Check IDs this sensor can emit |
| `scopes` | `array` of `string` | No | Scopes covered (e.g., `"workspace"`, `"crate"`) |
| `partial` | `boolean` | No | True if some inputs could not be processed |
| `reason` | `string` | No | Reason for partial results |

### JSON Example

```json
{
  "check_ids": [
    "licenses.missing",
    "licenses.unlicensed",
    "bans.duplicate",
    "bans.circular"
  ],
  "scopes": ["workspace"],
  "partial": true,
  "reason": "Some crates could not be parsed due to syntax errors"
}
```

### No Green By Omission

The capabilities block allows buildfix to distinguish between:

1. **No issues found**: Sensor ran completely and found nothing
2. **Incomplete scan**: Sensor couldn't check everything (`partial: true`)

This prevents false confidence when a sensor fails to run completely.

---

## Provenance Structure

Describes how a finding was derived, enabling trust decisions.

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `method` | `string` | Yes | Derivation method (e.g., `"dead_code_analysis"`) |
| `tools` | `array` of `string` | No | Tools/sensors that contributed |
| `agreement` | `boolean` | No | Whether multiple tools agree |
| `evidence_chain` | `array` of [`Evidence`](#evidence-structure) | No | Chain of evidence |

### Evidence Structure

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `source` | `string` | Yes | Source type (see below) |
| `value` | `any` | Yes | The evidence value |
| `validated` | `boolean` | No | Whether this evidence was validated |

### Evidence Sources

| Source | Description |
|--------|-------------|
| `repo` | Direct from repository |
| `lockfile` | From Cargo.lock |
| `registry` | From crates.io or other registry |
| `workspace` | Computed across workspace |
| `analysis` | From static analysis |

### JSON Example

```json
{
  "method": "license_detection",
  "tools": ["cargo-deny", "cargo-license"],
  "agreement": true,
  "evidence_chain": [
    {
      "source": "repo",
      "value": "MIT OR Apache-2.0",
      "validated": true
    },
    {
      "source": "registry",
      "value": { "license": "MIT/Apache-2.0" },
      "validated": true
    }
  ]
}
```

---

## FindingContext Structure

Additional context metadata for a finding.

### Field Reference

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `workspace` | [`WorkspaceContext`](#workspacecontext-structure) | No | Workspace-wide consensus data |
| `analysis_depth` | [`AnalysisDepth`](#analysisdepth-enum) | No | Depth of analysis performed |

### WorkspaceContext Structure

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `consensus_value` | `any` | No | Consensus value across workspace |
| `consensus_count` | `integer` | No | Number of crates with consensus |
| `total_crates` | `integer` | No | Total crates analyzed |
| `outliers` | `array` | No | Values differing from consensus |
| `outlier_crates` | `array` of `string` | No | Crates with outlier values |
| `all_crates_agree` | `boolean` | No | Whether all crates agree |

### AnalysisDepth Enum

| Value | Description |
|-------|-------------|
| `shallow` | Quick scan, may have false negatives |
| `full` | Standard analysis (default) |
| `deep` | Comprehensive with cross-referencing |

### JSON Example

```json
{
  "workspace": {
    "consensus_value": "2021",
    "consensus_count": 8,
    "total_crates": 10,
    "outliers": ["2018"],
    "outlier_crates": ["legacy-crate"],
    "all_crates_agree": false
  },
  "analysis_depth": "full"
}
```

---

## Check IDs and Conventions

Check IDs follow a hierarchical naming convention:

### Pattern

```
<category>.<subcategory>.<specific>
```

### Common Check IDs

| Check ID | Description | Sensor |
|----------|-------------|--------|
| `licenses.missing` | Missing license field | cargo-deny |
| `licenses.unlicensed` | No valid license detected | cargo-deny |
| `bans.duplicate` | Duplicate dependency versions | cargo-deny, cargo-tree |
| `bans.circular` | Circular dependencies | cargo-deny |
| `deps.unused_dependency` | Unused direct dependency | cargo-machete, cargo-udeps |
| `deps.unused_build_dependency` | Unused build dependency | cargo-udeps |
| `deps.multiple_versions` | Multiple versions of same crate | cargo-deny |
| `resolver.v1` | Workspace using resolver v1 | depguard |
| `path_dep.missing_version` | Path dependency without version | depguard |
| `workspace.inheritance` | Should use workspace inheritance | depguard |
| `msrv.incompatible` | MSRV incompatible with toolchain | cargo-msrv |
| `edition.outdated` | Edition not at latest | custom |

---

## Versioning Strategy

### Schema Versioning

Receipt schemas follow semantic versioning within the schema string:

```
sensor.report.v1
cargo-deny.report.v1
cargo-udeps.report.v1
```

### Compatibility Rules

1. **Additive changes**: New optional fields can be added without version bump
2. **Breaking changes**: Removing fields or changing semantics requires a new version
3. **Backward compatibility**: Adapters should support multiple schema versions

### Adapter Version Support

Adapters implement `AdapterMetadata` to declare supported schemas:

```rust
impl AdapterMetadata for CargoDenyAdapter {
    fn supported_schemas(&self) -> &[&str] {
        &["cargo-deny.report.v1", "cargo-deny.report.v2"]
    }
}
```

### Handling Format Changes

When a sensor's output format changes:

1. **Minor changes**: Update adapter to handle both formats
2. **Major changes**: Create new schema version
3. **Deprecation**: Support old schema for at least one major release

---

## Adapter Implementation

### Adapter Trait

```rust
pub trait Adapter: Send + Sync {
    fn sensor_id(&self) -> &str;
    fn load(&self, path: &Path) -> Result<ReceiptEnvelope, AdapterError>;
}
```

### Using ReceiptBuilder

The `ReceiptBuilder` provides a fluent API for constructing receipts:

```rust
let receipt = ReceiptBuilder::new("my-sensor")
    .with_schema("my-sensor.report.v1")
    .with_tool_version("1.0.0")
    .with_status(VerdictStatus::Fail)
    .with_counts(5, 2, 3)
    .with_check_id("category.check_name")
    .with_scope("workspace")
    .with_finding_at("Cargo.toml", 10, "Issue found", Severity::Error)
    .build();
```

### Error Handling

```rust
pub enum AdapterError {
    Io(std::io::Error),           // File read errors
    Json(serde_json::Error),      // Parse errors
    InvalidFormat(String),        // Unexpected format
    MissingField(String),         // Required field missing
}
```

---

## Domain Layer Consumption

The domain layer consumes receipts through the `ReceiptSet` type:

### ReceiptSet API

```rust
impl ReceiptSet {
    /// Create from loaded receipts
    pub fn from_loaded(loaded: &[LoadedReceipt]) -> Self;

    /// Find matching findings by tool, check_id, and code
    pub fn matching_findings(
        &self,
        tool_prefixes: &[&str],
        check_ids: &[&str],
        codes: &[&str],
    ) -> Vec<FindingRef>;

    /// Find matching findings with full data
    pub fn matching_findings_with_data(
        &self,
        tool_prefixes: &[&str],
        check_ids: &[&str],
        codes: &[&str],
    ) -> Vec<MatchedFinding>;
}
```

### Fixer Integration

Fixers query receipts using the `ReceiptSet`:

```rust
impl Fixer for MyFixer {
    fn plan(&self, ctx: &PlanContext, repo: &dyn RepoView, receipts: &ReceiptSet) 
        -> Result<Vec<PlanOp>> 
    {
        let triggers = receipts.matching_findings(
            Self::SENSORS,      // Tool prefixes
            Self::CHECK_IDS,    // Check IDs
            &[],                // Codes (empty = all)
        );
        
        if triggers.is_empty() {
            return Ok(Vec::new());
        }
        
        // Generate operations based on findings
        // ...
    }
}
```

---

## Complete Examples by Adapter

### cargo-deny

**Native Input:**
```json
{
  "licenses": {
    "deny": [
      {
        "id": "unlicensed",
        "message": "no license field was found",
        "package": {"name": "my-crate", "version": "0.1.0"}
      }
    ]
  },
  "bans": {
    "deny": [
      {
        "id": "multi-usage",
        "message": "package chrono 0.4.31 is used multiple times",
        "package": {"name": "chrono", "version": "0.4.31"}
      }
    ]
  }
}
```

**Converted Receipt:**
```json
{
  "schema": "cargo-deny.report.v1",
  "tool": {"name": "cargo-deny", "version": "0.14.0"},
  "verdict": {"status": "fail", "counts": {"findings": 2, "errors": 2, "warnings": 0}},
  "findings": [
    {
      "severity": "error",
      "check_id": "licenses.unlicensed",
      "message": "no license field was found",
      "data": {"name": "my-crate", "package": {"name": "my-crate", "version": "0.1.0"}}
    },
    {
      "severity": "error",
      "check_id": "bans.duplicate",
      "message": "package chrono 0.4.31 is used multiple times",
      "data": {"name": "chrono", "package": {"name": "chrono", "version": "0.4.31"}}
    }
  ]
}
```

### cargo-udeps

**Native Input:**
```json
{
  "success": true,
  "packages": [
    {
      "manifestPath": "/path/to/Cargo.toml",
      "name": "unused-crate",
      "version": "0.1.0",
      "kind": ["Normal"]
    }
  ]
}
```

**Converted Receipt:**
```json
{
  "schema": "cargo-udeps.report.v1",
  "tool": {"name": "cargo-udeps", "version": "0.1.45"},
  "verdict": {"status": "warn", "counts": {"findings": 1, "errors": 0, "warnings": 1}},
  "findings": [
    {
      "severity": "warn",
      "check_id": "deps.unused_dependency",
      "message": "unused unused-crate:0.1.0",
      "location": {"path": "/path/to/Cargo.toml"},
      "data": {"name": "unused-crate", "version": "0.1.0", "kind": ["Normal"]}
    }
  ]
}
```

### cargo-machete

**Native Input:**
```json
{
  "crates": [
    {"name": "unused-crate", "manifest_path": "/path/to/Cargo.toml", "kind": "direct"}
  ]
}
```

**Converted Receipt:**
```json
{
  "schema": "cargo-machete.report.v1",
  "tool": {"name": "cargo-machete"},
  "verdict": {"status": "warn", "counts": {"findings": 1, "errors": 0, "warnings": 1}},
  "findings": [
    {
      "severity": "warn",
      "check_id": "deps.unused_dependency",
      "message": "unused dependency: unused-crate",
      "location": {"path": "/path/to/Cargo.toml"},
      "data": {"name": "unused-crate", "kind": "direct"}
    }
  ]
}
```

### depguard

**Native Input:**
```json
{
  "files": [
    {
      "path": "crates/foo/Cargo.toml",
      "messages": [
        {"message": "path dependency bar should have a version", "code": "E001", "type": "path_requires_version", "line": 10}
      ]
    }
  ]
}
```

**Converted Receipt:**
```json
{
  "schema": "depguard.report.v1",
  "tool": {"name": "depguard"},
  "verdict": {"status": "fail", "counts": {"findings": 1, "errors": 1, "warnings": 0}},
  "findings": [
    {
      "severity": "error",
      "check_id": "path_dep.missing_version",
      "code": "E001",
      "message": "path dependency bar should have a version",
      "location": {"path": "crates/foo/Cargo.toml", "line": 10}
    }
  ]
}
```

### SARIF

**Native Input (SARIF 2.1.0):**
```json
{
  "version": "2.1.0",
  "runs": [{
    "tool": {"driver": {"name": "CodeAnalyzer", "version": "2.3.1"}},
    "results": [{
      "ruleId": "SAST001",
      "level": "error",
      "message": {"text": "SQL injection vulnerability"},
      "locations": [{
        "physicalLocation": {
          "artifactLocation": {"uri": "src/database/query.rs"},
          "region": {"startLine": 127, "startColumn": 15}
        }
      }]
    }]
  }]
}
```

**Converted Receipt:**
```json
{
  "schema": "sensor.report.v1",
  "tool": {"name": "CodeAnalyzer", "version": "2.3.1"},
  "verdict": {"status": "fail", "counts": {"findings": 1, "errors": 1, "warnings": 0}},
  "findings": [
    {
      "severity": "error",
      "check_id": "SAST001",
      "message": "SQL injection vulnerability",
      "location": {"path": "src/database/query.rs", "line": 127, "column": 15}
    }
  ]
}
```

---

## Best Practices

### For Adapter Authors

1. **Preserve native data**: Include original tool output in the `data` field
2. **Use consistent check_ids**: Follow the hierarchical naming convention
3. **Set appropriate severity**: Map tool severity to `error`/`warn`/`info`
4. **Include capabilities**: Help buildfix understand what was checked
5. **Handle partial runs**: Set `partial: true` when the scan was incomplete

### For Fixer Authors

1. **Query by check_id**: Use specific check IDs rather than tool names
2. **Handle empty results**: Always check if `matching_findings` is empty
3. **Use finding data**: Extract tool-specific information from `data` field
4. **Consider confidence**: Factor in confidence scores for safety classification

### For Sensor Integration

1. **Output to artifacts**: Write receipts to `artifacts/<sensor>/report.json`
2. **Use standard schema**: Prefer `sensor.report.v1` for custom sensors
3. **Include git context**: Add `git_head_sha` for reproducibility
4. **Timestamp runs**: Include `started_at` and `ended_at`

---

## Schema Registry

### Standard Schemas

| Schema | Description |
|--------|-------------|
| `sensor.report.v1` | Universal sensor envelope |
| `cargo-deny.report.v1` | cargo-deny adapter |
| `cargo-udeps.report.v1` | cargo-udeps adapter |
| `cargo-machete.report.v1` | cargo-machete adapter |
| `depguard.report.v1` | depguard adapter |
| `cargo-msrv.report.v1` | cargo-msrv adapter |
| `cargo-outdated.report.v1` | cargo-outdated adapter |
| `cargo-tree.report.v1` | cargo-tree adapter |

### Buildfix Internal Schemas

| Schema | Description |
|--------|-------------|
| `buildfix.plan.v1` | Repair plan output |
| `buildfix.apply.v1` | Apply result output |
| `buildfix.report.v1` | Report output |

---

## See Also

- [Adapter SDK Documentation](../../buildfix-adapter-sdk/README.md)
- [Fixer API Documentation](../../buildfix-fixer-api/README.md)
- [Architecture Overview](../architecture.md)
