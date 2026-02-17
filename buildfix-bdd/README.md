# buildfix-bdd

Cucumber acceptance test harness for buildfix.

This crate validates end-to-end behavior across planning, apply safety gates, determinism, and CLI surfaces.

## Layout

- `features/plan_and_apply.feature`: executable workflow scenarios
- `tests/cucumber.rs`: step definitions and test world

## Run

```bash
cargo test -p buildfix-bdd --test cucumber
```

## Scope

- Exercises the `buildfix` binary via `assert_cmd`
- Validates produced artifacts (`plan.json`, `apply.json`, `report.json`, patches)
- Covers policy blocks, unsafe/guarded gating, and idempotency

This crate is internal to the workspace (`publish = false`).
