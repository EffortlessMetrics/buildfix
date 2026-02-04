# buildfix-render

Markdown rendering for buildfix artifacts. Produces human-readable reports from repair plans and apply results.

## Functions

### `render_plan_md(plan: &BuildfixPlan) -> String`
Renders a plan as markdown:
- Summary counts (ops_total, ops_blocked, files_touched)
- List of ops with safety, policy keys, and findings
- Operation details per op

### `render_apply_md(apply: &BuildfixApply) -> String`
Renders apply results:
- Attempted/Applied/Blocked/Failed counts
- Per-op results with status and file changes
- Error details for failed ops

## Output Example

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
```

## Usage

```rust
use buildfix_render::{render_plan_md, render_apply_md};

let md = render_plan_md(&plan);
std::fs::write("plan.md", md)?;
```

This crate is part of the [buildfix](https://github.com/EffortlessMetrics/buildfix) workspace.
