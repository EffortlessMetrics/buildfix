# buildfix-fuzz

`cargo-fuzz` harnesses for parser and transform hardening.

This crate is intentionally excluded from workspace members and is used only for fuzzing.

## Targets

- `apply_op`
- `receipt_parse`
- `operation_apply`
- `plan_parse`
- `full_pipeline`

## Usage

```bash
cargo install cargo-fuzz
cargo fuzz run apply_op
```

Use these targets to stress receipt parsing, operation transforms, and plan/apply invariants.
