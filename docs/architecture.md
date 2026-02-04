# Architecture

buildfix is split into small crates with clear responsibilities:

## Crate Overview

```
buildfix-types      Shared DTOs and schemas (wire format)
       ↓
buildfix-receipts   Tolerant receipt loader
       ↓
buildfix-domain     Core planning logic (what to fix)
       ↓
buildfix-edit       TOML editing engine (how to fix)
       ↓
buildfix-render     Markdown artifact rendering
       ↓
buildfix-cli        CLI entry point
```

## Crate Responsibilities

### buildfix-types
Wire format definitions for all buildfix artifacts. Intentionally conservative with schema changes.

**Key types:**
- `BuildfixPlan`, `PlanOp`, `FilePrecondition` - Plan structure
- `OpKind` - Tagged enum of edit operations
- `SafetyClass` - Safe/Guarded/Unsafe classification
- `ReceiptEnvelope`, `Finding` - Receipt format
- `BuildfixApply`, `ApplyResult` - Apply results

### buildfix-receipts
Tolerant loader that reads `artifacts/*/report.json`. Collects errors without failing, sorts results deterministically.

### buildfix-domain
Core planning logic. Decides *what* should change based on receipts.

**Key abstractions:**
- `RepoView` trait - Read-only repo access (enables testing)
- `Fixer` trait - Individual fix implementation
- `Planner` - Orchestrates fixers to produce plans

**Built-in fixers:**
- `ResolverV2Fixer` - Workspace resolver = "2"
- `PathDepVersionFixer` - Add version to path deps
- `WorkspaceInheritanceFixer` - Use workspace = true
- `MsrvNormalizeFixer` - Normalize MSRV

### buildfix-edit
TOML editing engine using `toml_edit`. Decides *how* to modify files.

**Key functions:**
- `attach_preconditions()` - Add SHA256 + git HEAD checks
- `preview_patch()` - Generate diff without writing
- `apply_plan()` - Execute plan with optional backups

### buildfix-render
Markdown rendering for `plan.md` and `apply.md` artifacts.

### buildfix-cli
CLI entry point wiring clap + all modules. Subcommands: `plan`, `apply`, `explain`, `list-fixes`, `validate`.

### buildfix-bdd
Cucumber BDD tests for end-to-end workflow contracts.

### xtask
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
