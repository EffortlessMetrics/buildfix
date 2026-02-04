# buildfix — Safety Model

This document is normative: it describes guarantees `buildfix` must uphold.

## Core stance

`buildfix` is a torque wrench:

- narrow scope
- deterministic behavior
- refuses ambiguous edits
- produces audit artifacts

If `buildfix` becomes “helpful,” it becomes untrustworthy.

## Safety classes

Each planned operation (op) has a safety class:

### safe
- Inputs are fully determined from repo-local truth.
- There is exactly one correct result.
- Applying the operation does not require human judgment.

Examples:
- Insert or update `[workspace].resolver = "2"` in the workspace root.
- Set a path dependency version by reading the target crate’s `package.version`.

### guarded
- The operation is deterministic but higher impact, or may have workflow implications.
- Requires explicit opt-in to apply.

Examples:
- Normalizing many member manifests in a large workspace.
- Transformations that rewrite multiple tables but preserve semantics.

### unsafe
- The operation requires user choice or extra parameters.
- Plan-only unless required parameters are supplied explicitly.

Examples:
- “Choose a version” when multiple possible sources exist.
- Resolving conflicts between multiple MSRV declarations without a declared standard.

## Enforced gates

### No writes without explicit apply
- `buildfix plan` never writes repo files.
- `buildfix apply` requires explicit intent and may refuse by policy.

### Preconditions verification
- Plan includes file digests for all target files.
- Apply verifies all digests match.
- If mismatch: apply MUST stop with a policy block (exit 2) and write an apply.json indicating no changes were made.

### Dirty working tree
Default: refuse to apply on a dirty tree.
- Users can override with `--allow-dirty`, but the plan/apply artifacts must record this.

### Allowlist/denylist
- Policy keys are derived from triggers as `sensor/check_id/code`.
- If a policy key is denied: the op is blocked.
- If allowlist is non-empty: only allowlisted policy keys are eligible.
- Denials are recorded in the plan as blocked ops with reasons.

### Caps
Plan MUST enforce reasonable caps by default:
- max operations
- max files touched
- max patch size
Caps are policy blocks, not tool errors.

## Audit trail invariants

`buildfix` MUST produce:
- a plan
- a patch preview
- an apply record (when apply is attempted)

`buildfix` SHOULD:
- write backups of modified files (configurable, on by default)
- record exact tool version and commit id in artifacts

## “Never invent” rule

When an op needs a value:
- It must be derived from repo-local truth **or**
- It must be provided explicitly by the user as a parameter (unsafe)

If neither is true, the fix remains blocked and is marked unsafe.
