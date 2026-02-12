# buildfix — Testing Strategy

buildfix is a gatekeeper tool that writes to repositories. The test bar should be higher than for pure sensors.

This plan uses four layers:

1) BDD (behavioral contracts)
2) Golden fixtures (deterministic outputs)
3) Property tests (semantic invariants)
4) Fuzz + mutation (resilience + correctness pressure)

## 1) BDD (Gherkin)
BDD expresses the safety posture and user workflow: plan/apply, preconditions, allowlists, unsafe blocks.

Files live in:
- `buildfix-bdd/features/*.feature` - Gherkin scenarios
- `buildfix-bdd/tests/cucumber.rs` - Step definitions

The BDD harness uses `cucumber-rs` and `assert_cmd` to invoke the CLI in isolated temp directories. Step definitions create workspaces, synthetic receipts, run commands, and validate outputs.

## 2) Golden fixtures (the determinism anchor)
For each fixer:

Fixture contains:
- a small repo snapshot (files under `tests/fixtures/<name>/repo/`)
- a set of receipts under `tests/fixtures/<name>/receipts/`
Expected outputs:
- `expected/plan.json`
- `expected/patch.diff`
- `expected/apply.json` (when apply is tested)
- optional `expected/report.json`

Golden tests:
- run plan twice → byte-identical outputs
- apply → exact expected file content + apply.json

## 3) Property tests (proptest)
Focus on invariants that prevent “semantic drift”:

- TOML editing preserves unrelated keys and formatting best effort
- Workspace inheritance transform preserves allowed flags (`features`, `optional`, etc.)
- Ordering is stable regardless of input ordering of receipts and findings
- Preconditions mismatch always blocks and produces no writes

## 4) Fuzz testing
Two cheap, high ROI fuzz surfaces:

- Receipt ingestion (JSON parsing + normalization)
- TOML transform operations (never panic, never corrupt; fail safely)

Targets:
- `fuzz_receipt_parse`
- `fuzz_toml_transform`

## 5) Mutation testing (cargo-mutants)
Run mutants on domain crates:
- policy evaluation
- planner routing + ordering
- precondition verification logic
- safety gating decisions

Mutation runs are typically scheduled (nightly) to keep PR latency reasonable.

## CI gates (recommended)
- `cargo test` (all platforms)
- schema validation (plan/apply/report json against schemas)
- golden fixture tests
- `cargo fmt` + `clippy`
- fuzz target builds (at least compile)
- scheduled: `cargo mutants` and long fuzz runs

## What we do NOT test by default
- We don't need exhaustive integration against real-world huge workspaces in unit CI.
Instead, add a small curated set of “realistic” fixtures and rely on determinism + safety gates.
