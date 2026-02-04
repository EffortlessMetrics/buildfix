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
- Summary counts (total fixes, by safety class)
- List of fixes with title, id, safety, triggers
- Operation details per fix

### `render_apply_md(apply: &BuildfixApply) -> String`
Renders apply results as markdown with:
- Attempted/Applied/Skipped/Failed counts
- Per-fix results with status and file changes
- Error details for failed fixes

## Output Format

```markdown
# Buildfix Plan

## Summary
- Total fixes: 3
- Safe: 2, Guarded: 1, Unsafe: 0

## Fixes

### 1. Add workspace resolver v2
- **ID:** `abc123`
- **Safety:** Safe
- **Triggers:** builddiag/resolver-missing
...
```

## Invariants

- Deterministic output for same input
- No external dependencies beyond buildfix-types
- Markdown compatible with GitHub rendering
