# buildfix-types

Shared DTOs and schemas for buildfix repair plans. This crate defines the wire format for all buildfix artifacts.

## Key Types

### Safety Classification
- `SafetyClass` - `Safe`, `Guarded`, `Unsafe` classification for ops

### Plan Types
- `BuildfixPlan` - Complete repair plan with ops, inputs, and policy
- `PlanOp` - Individual op with target, kind, rationale, and safety class
- `FilePrecondition` - File SHA256 preconditions

### Operations
- `OpKind` - Tagged enum of supported operation kinds:
  - `toml_set`
  - `toml_remove`
  - `toml_transform` (rule_id + args)

### Receipt Types
- `ReceiptEnvelope` - Generic sensor receipt format
- `Finding` - Individual finding with location, severity, message

### Apply Types
- `BuildfixApply` - Results of applying a plan
- `ApplyResult` - Per-op outcome

## Schema Versions

- `BUILDFIX_PLAN_V1`
- `BUILDFIX_APPLY_V1`
- `BUILDFIX_REPORT_V1`

## Usage

```rust
use buildfix_types::plan::BuildfixPlan;
use buildfix_types::ops::{OpKind, SafetyClass};
```

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
