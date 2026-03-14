# buildfix-types

Shared data contracts for the buildfix workspace.

This crate defines the canonical Rust models and schema IDs for plan/apply/report artifacts and receipt envelopes.

## Modules

- `ops`: operation kinds, targets, and safety classes
- `plan`: plan document and operation rationale/preconditions
- `apply`: apply results, status, and file-level outcomes
- `report`: canonical sensor-compatible report model
- `receipt`: tolerant sensor receipt envelope model
- `wire`: wire-format conversion helpers for schema-stable JSON

## Schema identifiers

- `buildfix.plan.v1`
- `buildfix.apply.v1`
- `buildfix.report.v1`
- `sensor.report.v1`

## Design constraints

- Backward compatibility for serialized artifacts
- Additive evolution preferred over breaking field changes
- Explicit serde defaults for tolerant parsing

This is a support crate for the `buildfix` workspace and may evolve in lockstep with the workspace release train.
