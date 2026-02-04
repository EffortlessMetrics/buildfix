# buildfix — Design

## Design goals
- **Safety-first**: refuse ambiguous edits; never invent values; never write without explicit apply.
- **Receipts-first**: depend on stable receipt semantics, not sensor internals.
- **Deterministic**: byte-stable plans and patches for the same inputs.
- **Explainable**: every operation has a reason chain to findings.
- **Minimal diffs**: edits change only what is required.
- **Composable**: policy is external (buildfix.toml, cockpit.toml); buildfix emits audit artifacts.

## Architectural style
Hexagonal / ports-and-adapters (“clean architecture”):
- Domain is pure logic (planner + safety policy + fix registry)
- Adapters handle filesystem, git identity, schema validation, patch writing
- CLI is just wiring

## Key objects

### NormalizedFinding
A stable internal representation for routing and planning.

Fields:
- sensor_id (from receipt tool.name)
- check_id (optional)
- code
- severity
- location (path, line?)
- message
- hint (finding.data; optional)

### FixKey
Routing identifier, treated as API for allow/deny and explain:
- `sensor_id/check_id/code` (preferred)
- `sensor_id/*/code` fallback patterns supported for policy matching

### FixCandidate
Output of `probe`:
- fix_key
- safety_class (initial)
- required_files
- required_params (for unsafe cases)
- summary (human-facing)

### Operation (Op)
The smallest reversible action.

v0 Op vocabulary:
- TomlSet(file, toml_path, value)
- TomlRemove(file, toml_path)
- TomlTransform(file, rule_id, args)

Op invariants:
- deterministic: same inputs → same op list
- minimal: no reformatting churn
- reversible: backup + patch preview exist

### Plan
The plan is the “contract of intent.”

It contains:
- tool identity and run metadata
- list of receipts consumed
- effective policy snapshot (allow/deny, caps, safety gates)
- precondition snapshot (file digests + best-effort git identity)
- ordered ops, each with:
  - safety class
  - blocked flag + reason (if blocked)
  - rationale chain to findings

### ApplyResult
Evidence of execution:
- preconditions verified?
- ops applied/blocked/failed
- per-file before/after digests
- backup paths
- errors (if any)

## Algorithm

### Plan
1. Discover receipts (glob under artifacts; ignore cockpit/buildfix directories).
2. Parse receipts and normalize findings.
3. Route normalized findings to fixers by fix key.
4. Each fixer probes applicability and produces candidates.
5. Apply policy:
   - denylist
   - allowlist
   - safety gates
   - caps
6. For eligible candidates, produce ops (pure).
7. Sort ops deterministically.
8. Compute patch preview (read-only, via editor + diff renderer).
9. Emit plan.json + plan.md + patch.diff.

### Apply
1. Load plan.json.
2. Verify preconditions:
   - file digests match
   - git head sha matches if captured (best-effort)
3. Enforce policy gates again (don’t trust old plan if policy changed).
4. Apply ops deterministically:
   - backup then atomic write
   - record per-op result
5. Emit apply.json (+ optional report.json envelope).

## Preconditions
Preconditions are there to prevent “plan on one state, apply on another.”

Minimum viable snapshot:
- sha256 digest per targeted file
- repo root path
- best-effort HEAD sha and dirty flag

If preconditions mismatch:
- treat as policy block (exit 2)
- write apply.json indicating no changes applied

## Path identity
Internally canonicalize paths:
- repo-relative, forward slashes
- no leading `./`
- ensure consistent behavior on Windows

## Explainability
buildfix should ship an explain registry:
- fix keys
- safety class rationale
- remediation guidance (what it will change, what it will not)

`buildfix explain` uses that registry.

## Extensibility rules
New fixers must satisfy:
- deterministic from repo-local truth (or clearly unsafe and param-gated)
- minimal diff footprint
- BDD scenario + golden fixture + proptest invariant where appropriate
- explicit non-goal note if it would expand scope beyond actuation

## Microcrate boundaries (internal)

- buildfix-types: DTOs + schema ids (wire format for all artifacts)
- buildfix-receipts: receipt envelope ingestion + normalization
- buildfix-domain: registry + policy + planner + ordering + Fixer trait
- buildfix-edit: TOML editing + diff preview generation + preconditions
- buildfix-render: plan.md + apply.md rendering
- buildfix-cli: clap + filesystem wiring + config loading + explain
- buildfix-bdd: cucumber BDD tests for workflow contracts
- xtask: schema regen, fixture management, artifact init
