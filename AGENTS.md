# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                          # Build all crates
cargo test                           # Run all tests
cargo test -p buildfix-domain        # Test a specific crate
cargo fmt                            # Format code
cargo clippy                         # Lint

# Run the CLI
cargo run -p buildfix -- plan        # Generate plan from receipts
cargo run -p buildfix -- apply       # Dry-run apply
cargo run -p buildfix -- apply --apply               # Apply safe ops
cargo run -p buildfix -- apply --apply --allow-guarded  # Include guarded ops
```

## Architecture

**buildfix** is a receipt-driven repair tool for Cargo workspace hygiene. It consumes sensor receipts (`artifacts/<sensor>/report.json`) and emits deterministic repair plans.

### Crate Responsibilities

**Intake Layer** (receipt adapters):
- **buildfix-adapter-sdk**: Adapter SDK with traits and test harness
- **buildfix-receipts-sarif**: Generic SARIF intake
- **buildfix-receipts-cargo-***: Cargo tool adapters (deny, machete, udeps, outdated, lock, update, tree, bloat, llvm-lines, cyclonedds, geiger, semver-checks, warn, msrv, krate, audit, sec-audit, audit-freeze, crev, miri, spellcheck, unused-function)
- **buildfix-receipts-clippy**: Clippy lint intake
- **buildfix-receipts-rustc-json**: rustc JSON message intake
- **buildfix-receipts-rustfmt**: rustfmt output intake
- **buildfix-receipts-depguard**: depguard intake
- **buildfix-receipts-tarpaulin**: tarpaulin coverage intake

**Core Types**:
- **buildfix-types**: Shared DTOs and schemas (`BuildfixPlan`, `PlanOp`, `OpKind`, `SafetyClass`)
- **buildfix-hash**: SHA256 utilities
- **buildfix-artifacts**: Artifact path management

**Fixer Layer** (each fixer is a microcrate):
- **buildfix-fixer-api**: Fixer trait + common types
- **buildfix-fixer-resolver-v2**: Workspace resolver = "2"
- **buildfix-fixer-path-dep-version**: Add version to path deps
- **buildfix-fixer-workspace-inheritance**: Use workspace = true
- **buildfix-fixer-duplicate-deps**: Consolidate duplicate dep versions
- **buildfix-fixer-remove-unused-deps**: Remove unused dependencies
- **buildfix-fixer-msrv**: Normalize MSRV
- **buildfix-fixer-edition**: Normalize edition
- **buildfix-fixer-license**: Normalize license
- **buildfix-fixer-catalog**: Registry of all built-in fixers

**Domain Layer**:
- **buildfix-domain**: Core planning logic with `Fixer` trait; decides *what* should change
- **buildfix-domain-policy**: Policy evaluation (allow/deny/caps)
- **buildfix-core**: Pipeline orchestration
- **buildfix-core-runtime**: Runtime adapters (filesystem, git)

**Output Layer**:
- **buildfix-edit**: TOML editing engine using `toml_edit`; decides *how* to modify files
- **buildfix-render**: Markdown rendering for plan/apply artifacts
- **buildfix-report**: Report generation
- **buildfix-cli**: CLI entry point wiring clap + IO

**Testing**:
- **buildfix-bdd**: Cucumber behavior tests
- **xtask**: Build helpers

### Data Flow

1. Receipts loaded from `artifacts/*/report.json`
2. Domain planner routes findings to fixers by fix key
3. Each fixer emits `PlanOp` operations with safety class
4. Edit engine attaches SHA256 preconditions and generates patch preview
5. Artifacts emitted: `plan.json`, `plan.md`, `patch.diff`, `report.json`

### Safety Model

Operations are classified by safety:
- **safe**: Fully determined from repo truth, auto-applied
- **guarded**: Deterministic but higher impact, requires `--allow-guarded`
- **unsafe**: Requires user parameters, plan-only

Key invariants:
- Preconditions: Plans include SHA256 hashes; apply verifies before writing
- Never invents values—must derive from repo or user provides explicitly
- Exit 0 = success, Exit 2 = policy block (precondition mismatch, denied fix), Exit 1 = tool error

### Testing Layers

1. **BDD**: Feature files in `tests/features/` for workflow contracts
2. **Golden fixtures**: `tests/fixtures/<name>/` with expected outputs for determinism
3. **Proptest**: Invariants (TOML preservation, stable ordering)
4. **Fuzz**: Receipt parsing and TOML transforms

### Key Patterns

- Hexagonal architecture: domain is testable independently via `RepoView` trait
- `Fixer` trait: `plan(ctx, repo, receipts) -> Vec<PlanOp>`
- Paths normalized: repo-relative, forward slashes, no leading `./`
- Deterministic sorting via `stable_fix_sort_key()` for byte-stable outputs

### Publishing Conventions

- Most sub-crates use `readme = false` in `Cargo.toml` because they have no individual README.md files. This is intentional -- it prevents `cargo package` failures. The workspace-root `README.md` serves as the project documentation.
- Crates not intended for crates.io (`buildfix-bdd`, `xtask`, `buildfix-receipts-template`) have `publish = false` in their `Cargo.toml`.
- The CI publish workflow uses `cargo-workspaces` which resolves dependency ordering automatically. See `docs/release-runbook.md` for manual publish order as a fallback.
