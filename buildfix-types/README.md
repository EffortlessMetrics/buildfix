# buildfix-types

Shared DTOs and schemas for buildfix repair plans. This crate defines the wire format for all buildfix artifacts.

## Key Types

### Safety Classification
- `SafetyClass` - `Safe`, `Guarded`, `Unsafe` classification for fixes

### Plan Types
- `BuildfixPlan` - Complete repair plan with fixes, receipts, and policy
- `PlannedFix` - Individual fix with operations, triggers, and safety class
- `Precondition` - `FileExists`, `FileSha256`, `GitHeadSha` preconditions

### Operations
- `Operation` - Tagged enum of all supported operations:
  - `EnsureWorkspaceResolverV2`
  - `EnsurePathDepHasVersion`
  - `UseWorkspaceDependency`
  - `SetPackageRustVersion`

### Receipt Types
- `ReceiptEnvelope` - Generic sensor receipt format
- `Finding` - Individual finding with location, severity, message

### Apply Types
- `BuildfixApply` - Results of applying a plan
- `AppliedFixResult` - Per-fix outcome

## Schema Versions

- `BUILDFIX_PLAN_V1`
- `BUILDFIX_APPLY_V1`
- `BUILDFIX_REPORT_V1`

## Usage

```rust
use buildfix_types::plan::BuildfixPlan;
use buildfix_types::ops::{Operation, SafetyClass};
```

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
