# buildfix-render

Markdown rendering for buildfix artifacts. Produces human-readable reports from repair plans and apply results.

## Functions

### `render_plan_md(plan: &BuildfixPlan) -> String`
Renders a plan as markdown:
- Summary counts (total fixes, by safety class)
- List of fixes with title, ID, safety, triggers
- Operation details per fix

### `render_apply_md(apply: &BuildfixApply) -> String`
Renders apply results:
- Attempted/Applied/Skipped/Failed counts
- Per-fix results with status and file changes
- Error details for failed fixes

## Output Example

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
```

## Usage

```rust
use buildfix_render::{render_plan_md, render_apply_md};

let md = render_plan_md(&plan);
std::fs::write("plan.md", md)?;
```

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
