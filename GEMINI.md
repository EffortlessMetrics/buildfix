# buildfix

**buildfix** is a receipt-driven repair tool for Cargo workspace hygiene. It ingests "receipts" (JSON reports from sensors like `buildscan`, `builddiag`, `depguard`) and produces deterministic, safe plans to fix issues.

## Project Overview

This is a Rust workspace composed of several crates, following a Hexagonal Architecture pattern to separate domain logic from IO and editing mechanisms.

### Key Characteristics
*   **Receipt-Driven**: Does not scan code itself; relies on external sensor reports.
*   **Deterministic**: Given the same inputs (repo state + receipts), it produces the exact same plan.
*   **Safety-First**: Operations are classified as `safe`, `guarded`, or `unsafe`.
*   **Audit-Ready**: Produces plans, patch previews, and apply records.
*   **"Never Invent"**: Values are derived from repo truth or user parameters, never guessed.

## Architecture & Crates

*   `buildfix-cli`: The CLI entry point (`src/main.rs`). Wires `clap` commands to the domain.
*   `buildfix-domain`: Core planning logic. Contains `Fixer` traits and implementation.
*   `buildfix-edit`: The editing engine using `toml_edit`. Handles preserving comments/formatting.
*   `buildfix-receipts`: Tolerant loader for input reports (`artifacts/*/report.json`).
*   `buildfix-types`: Shared DTOs (Plan, Apply, Report, Operations) and Schemas.
*   `buildfix-render`: Markdown rendering for artifacts (`plan.md`, `apply.md`).
*   `buildfix-bdd`: Cucumber test harness for behavior-driven testing.
*   `xtask`: Workspace automation tasks.

## Development Workflow

### Build & Test
Standard Cargo commands are used.

```bash
# Build the entire workspace
cargo build

# Run all tests (Unit + BDD + Doc)
cargo test

# Run specific crate tests
cargo test -p buildfix-domain

# Run BDD/Feature tests specifically
# These are located in `buildfix-bdd/tests/cucumber.rs`
cargo test -p buildfix-bdd
```

### Linting & Formatting
```bash
cargo fmt
cargo clippy
```

### Running the Tool (Dev Mode)
Use `cargo run` to execute the CLI from source.

```bash
# Generate a plan (needs receipts in artifacts/)
cargo run -p buildfix -- plan

# Dry-run apply (generates preview but writes nothing)
cargo run -p buildfix -- apply

# Apply safe changes
cargo run -p buildfix -- apply --apply

# Apply including 'guarded' (high impact) changes
cargo run -p buildfix -- apply --apply --allow-guarded
```

## Safety Model

Understanding the safety model is critical when modifying this tool.

1.  **Safe**: Fully determined from repo truth. Auto-applied.
    *   *Example*: Setting `[workspace].resolver = "2"`.
2.  **Guarded**: Deterministic but high impact/workflow implication. Requires `--allow-guarded`.
    *   *Example*: Normalizing MSRV across many crates.
3.  **Unsafe**: Ambiguous or requires user parameters. Plan-only by default.
    *   *Example*: Choosing a version when multiple exist.

**Invariants:**
*   **Preconditions**: Every plan operation includes SHA256 hashes of the target files. `apply` MUST verify these match before writing.
*   **No Writes without `--apply`**: `plan` is read-only. `apply` defaults to dry-run.

## Key Code Patterns

*   **Hexagonal Architecture**: The `domain` crate uses a `RepoView` trait to abstract file access, making it testable without real disk IO.
*   **Fixer Trait**: `plan(ctx, repo, receipts) -> Vec<PlanOp>`.
*   **Path Normalization**: All paths are repo-relative, use forward slashes, and have no leading `./`.
*   **Stable Sorting**: Output lists are sorted by a deterministic key to ensure byte-stable artifact generation.

## Directory Structure Highlights

*   `artifacts/`: Default location for input receipts and output plans (ignored by git).
*   `tests/features/`: Gherkin feature files describing scenarios.
*   `tests/fixtures/`: Golden test data for ensuring output stability.
