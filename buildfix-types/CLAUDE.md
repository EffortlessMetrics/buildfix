# buildfix-types

Shared DTOs and schemas for serialization to disk. This crate is intentionally conservative with schema changes since artifacts are persisted and may be read across versions.

## Build & Test

```bash
cargo test -p buildfix-types
cargo clippy -p buildfix-types
```

## Key Types

### Safety Classification
- `SafetyClass` enum: `Safe`, `Guarded`, `Unsafe` - determines auto-apply behavior

### Plan Types (`plan.rs`)
- `BuildfixPlan` - Complete plan with fixes, receipts consumed, and policy info
- `PlannedFix` - Individual fix with id, title, operations, triggers, safety class
- `Precondition` enum: `FileExists`, `FileSha256`, `GitHeadSha`

### Operations (`ops.rs`)
- `Operation` enum - Tagged operations that can be applied:
  - `EnsureWorkspaceResolverV2`
  - `EnsurePathDepHasVersion`
  - `UseWorkspaceDependency`
  - `SetPackageRustVersion`

### Receipt Types (`receipt.rs`)
- `ReceiptEnvelope` - Generic sensor receipt with tool, check_id, findings
- `Finding` - Individual finding with location, message, severity, data

### Apply Types (`apply.rs`)
- `BuildfixApply` - Results of applying a plan
- `AppliedFixResult` - Per-fix outcome with status and file changes

## Schema Versions

Constants defined for forward compatibility:
- `BUILDFIX_PLAN_V1`
- `BUILDFIX_APPLY_V1`
- `BUILDFIX_REPORT_V1`

## Invariants

- All types derive `Serialize`, `Deserialize`, `Clone`, `Debug`
- Optional fields use `#[serde(default, skip_serializing_if)]` for clean JSON
- Schema changes must be backwards-compatible or bump version
