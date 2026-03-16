# buildfix-render

Markdown renderers for buildfix artifacts.

This crate turns structured `buildfix-types` data into human-readable markdown for operator and CI consumption.

## API

- `render_plan_md(&BuildfixPlan) -> String`
- `render_apply_md(&BuildfixApply) -> String`
- `render_comment_md(&BuildfixPlan) -> String`

## Output roles

- `plan.md`: detailed plan summary and operation listing
- `apply.md`: per-op apply results and file-change hashes
- `comment.md`: short cockpit/PR-friendly summary with artifact pointers

## Boundaries

- No planning logic
- No file mutation
- No schema validation

This is a support crate for the `buildfix` workspace and may evolve in lockstep with the workspace release train.
