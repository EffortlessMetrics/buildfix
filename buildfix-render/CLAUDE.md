# buildfix-render

Markdown rendering for human-readable artifacts.

## Build & Test

```bash
cargo test -p buildfix-render
cargo clippy -p buildfix-render
```

## Key Functions

### `render_plan_md(plan: &BuildfixPlan) -> String`
Renders a plan as markdown with:
- Summary counts (ops_total, ops_blocked, files_touched)
- List of ops with safety, policy keys, findings
- Operation details per op

### `render_apply_md(apply: &BuildfixApply) -> String`
Renders apply results as markdown with:
- Attempted/Applied/Blocked/Failed counts
- Per-op results with status and file changes
- Error details for failed ops

## Output Format

```markdown
# Buildfix Plan

## Summary
- Ops total: 3
- Ops blocked: 0
- Files touched: 2

## Ops

### 1. builddiag/workspace.resolver_v2/not_v2
- **ID:** `abc123`
- **Safety:** Safe
- **Findings:** builddiag/workspace.resolver_v2/not_v2
...
```

## Invariants

- Deterministic output for same input
- No external dependencies beyond buildfix-types
- Markdown compatible with GitHub rendering
