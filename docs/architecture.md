# Architecture

buildfix uses a microcrate architecture with clear separation of concerns. Each crate has a single responsibility and minimal dependencies.

## Crate Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              INTAKE LAYER                                    │
│  Receipt adapters translate sensor outputs → normalized findings             │
├─────────────────────────────────────────────────────────────────────────────┤
│ buildfix-adapter-sdk           Adapter SDK (traits + test harness)           │
│ buildfix-receipts-sarif        Generic SARIF intake                          │
│ buildfix-receipts-cargo-*      Cargo tool adapters (deny, machete, etc.)     │
│ buildfix-receipts-clippy       Clippy lint intake                            │
│ buildfix-receipts-rustc-json   rustc JSON message intake                     │
│ buildfix-receipts-rustfmt      rustfmt output intake                         │
│ buildfix-receipts-depguard     depguard intake                               │
│ buildfix-receipts-tarpaulin    tarpaulin coverage intake                     │
└─────────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CORE TYPES                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│ buildfix-types                 Shared DTOs and schemas (wire format)         │
│ buildfix-hash                  SHA256 utilities                              │
│ buildfix-artifacts             Artifact path management                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────────┐
│                              FIXER LAYER                                     │
│  Each fixer is an independent microcrate                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ buildfix-fixer-api             Fixer trait + common types                    │
│ buildfix-fixer-resolver-v2     Workspace resolver = "2"                      │
│ buildfix-fixer-path-dep-version  Add version to path deps                   │
│ buildfix-fixer-workspace-inheritance  Use workspace = true                  │
│ buildfix-fixer-duplicate-deps  Consolidate duplicate dep versions            │
│ buildfix-fixer-remove-unused-deps  Remove sensor-reported unused deps       │
│ buildfix-fixer-msrv            Normalize MSRV                                │
│ buildfix-fixer-edition         Normalize edition                             │
│ buildfix-fixer-license         Normalize package.license from workspace      │
│ buildfix-fixer-catalog         Registry of all built-in fixers               │
└─────────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────────┐
│                              DOMAIN LAYER                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ buildfix-domain                Core planning logic (what to fix)             │
│ buildfix-domain-policy         Policy evaluation (allow/deny/caps)           │
│ buildfix-core                  Pipeline orchestration                        │
│ buildfix-core-runtime          Runtime adapters (filesystem, git)            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────────┐
│                              OUTPUT LAYER                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│ buildfix-edit                  Deterministic edit engine (how to fix)        │
│ buildfix-render                Markdown artifact rendering                   │
│ buildfix-report                Report generation                             │
│ buildfix-cli                   CLI entry point                               │
└─────────────────────────────────────────────────────────────────────────────┘
                                    ↓
┌─────────────────────────────────────────────────────────────────────────────┐
│                              TESTING                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│ buildfix-bdd                   Cucumber BDD tests                            │
│ xtask                          Build helpers (print-schemas, init-artifacts) │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Crate Responsibilities

### Intake Layer

#### buildfix-adapter-sdk
SDK for writing intake adapters. Provides traits, test harness, and receipt builder utilities.

**Key types:**
- `IntakeAdapter` trait - Transform sensor output → normalized findings
- `ReceiptBuilder` - Build valid receipt structures
- Test harness for adapter validation

#### buildfix-receipts-cargo-*
Microcrates for each Cargo tool (deny, machete, udeps, outdated, lock, update, tree, bloat, llvm-lines, cyclonedds, geiger, semver-checks, warn, msrv, krate, audit, sec-audit, audit-freeze, crev, miri, spellcheck, unused-function).

Each adapter:
- Parses tool-specific output format
- Normalizes to standard `Finding` structure
- Provides test fixtures for validation

#### buildfix-receipts-sarif
Generic SARIF (Static Analysis Results Interchange Format) intake for tools emitting standard SARIF output.

#### buildfix-receipts-clippy
Clippy lint intake from JSON messages.

#### buildfix-receipts-rustc-json
rustc JSON message intake for edition, MSRV, and compilation findings.

#### buildfix-receipts-rustfmt
rustfmt diff/output intake.

#### buildfix-receipts-depguard
depguard dependency guard intake.

#### buildfix-receipts-tarpaulin
tarpaulin coverage report intake.

### Core Types

#### buildfix-types
Wire format definitions for all buildfix artifacts. Intentionally conservative with schema changes.

**Key types:**
- `BuildfixPlan`, `PlanOp`, `FilePrecondition` - Plan structure
- `OpKind` - Tagged enum of edit operations
- `SafetyClass` - Safe/Guarded/Unsafe classification
- `ReceiptEnvelope`, `Finding` - Receipt format
- `BuildfixApply`, `ApplyResult` - Apply results

#### buildfix-hash
SHA256 hashing utilities for precondition computation.

#### buildfix-artifacts
Artifact path management and discovery.

### Fixer Layer

#### buildfix-fixer-api
Core fixer trait and common types shared by all fixers.

**Key types:**
- `Fixer` trait - Individual fix implementation
- `FixerMeta` - Metadata (fix key, safety, sensors, check IDs)

#### buildfix-fixer-resolver-v2
Workspace resolver = "2" fixer.

#### buildfix-fixer-path-dep-version
Add version to path dependencies.

#### buildfix-fixer-workspace-inheritance
Use workspace = true for dependencies.

#### buildfix-fixer-duplicate-deps
Consolidate duplicate dependency versions.

#### buildfix-fixer-remove-unused-deps
Remove sensor-reported unused dependencies.

#### buildfix-fixer-msrv
Normalize MSRV across workspace.

#### buildfix-fixer-edition
Normalize Rust edition across workspace.

#### buildfix-fixer-license
Normalize package.license from workspace.

#### buildfix-fixer-catalog
Registry aggregating all built-in fixers.

### Domain Layer

#### buildfix-domain
Core planning logic. Decides *what* should change based on receipts.

**Key abstractions:**
- `RepoView` trait - Read-only repo access (enables testing)
- `Planner` - Orchestrates fixers to produce plans
- `ReceiptSet` - Normalized collection of findings

#### buildfix-domain-policy
Policy evaluation for allow/deny lists and capability caps.

#### buildfix-core
Pipeline orchestration connecting all layers.

#### buildfix-core-runtime
Runtime adapters implementing domain ports (filesystem, git operations).

### Output Layer

#### buildfix-edit
Deterministic edit engine for TOML, anchored text replacements, and mechanical
JSON/YAML path edits. Decides *how* to modify files.

**Key functions:**
- `attach_preconditions()` - Add SHA256 + git HEAD checks
- `preview_patch()` - Generate diff without writing
- `apply_plan()` - Execute plan with optional backups

#### buildfix-render
Markdown rendering for `plan.md` and `apply.md` artifacts.

#### buildfix-report
Report generation and aggregation.

#### buildfix-cli
CLI entry point wiring clap + all modules. Subcommands: `plan`, `apply`, `explain`, `list-fixes`, `validate`.

### Testing

#### buildfix-bdd
Cucumber BDD tests for end-to-end workflow contracts.

#### xtask
Build helpers: `print-schemas`, `init-artifacts`.

## Data Flow

```
1. Receipts loaded       artifacts/*/report.json
         ↓
2. Normalize findings    ReceiptSet (sorted)
         ↓
3. Route to fixers       Fixer.plan() for each
         ↓
4. Collect ops           Vec<PlanOp> (sorted, deduped)
         ↓
5. Add preconditions     SHA256 hashes, git HEAD
         ↓
6. Generate preview      patch.diff (unified diff)
         ↓
7. Emit artifacts        plan.json, plan.md, report.json
         ↓
8. Apply (optional)      Verify preconditions, write files
         ↓
9. Emit results          apply.json, apply.md
```

## Safety Model

Every planned op has a safety class:

| Class | Auto-apply | Description |
|-------|------------|-------------|
| **Safe** | Yes | Deterministic, low impact, single correct answer |
| **Guarded** | With flag | Deterministic but higher impact |
| **Unsafe** | With flag + params | Requires user input |

## Preconditions

Plans include SHA256 preconditions for each touched file. Apply refuses to run when the repo has drifted (unless explicitly overridden).

Precondition types:
- File SHA256 for each touched file
- Optional git HEAD SHA
- Optional dirty flag

## Determinism Guarantees

Same inputs always produce byte-identical outputs:
- Ops sorted by a stable op sort key (manifest + rule id + args fingerprint)
- Deterministic UUIDs via `Uuid::new_v5` hashing
- Receipts sorted by path
- Findings sorted by location/tool/check_id

## Path Normalization

Internally all paths are canonicalized:
- Repo-relative
- Forward slashes
- No leading `./`
- Consistent behavior on Windows
