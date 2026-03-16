# buildfix-edit

Edit engine for buildfix plans.

This crate applies planned operations, checks preconditions, enforces safety gates at apply time, and produces patch previews.

## Key APIs

- `attach_preconditions(...)`: add SHA256 file preconditions (and optional git HEAD precondition)
- `preview_patch(...)`: render unified diff without writing files
- `apply_plan(...)`: execute plan in dry-run or write mode and return `BuildfixApply`
- `check_policy_block(...)`: detect policy-block outcomes for exit-code mapping
- `apply_op_to_content(...)`: pure operation-to-content transform
- `execute_plan_from_contents(...)`: apply ops using in-memory content maps

## Supported op shapes

- `toml_set`
- `toml_remove`
- `toml_transform` (rule-based transforms)
- `text_replace_anchored`

## Built-in transform rules

- `ensure_workspace_resolver_v2`
- `set_package_rust_version`
- `set_package_edition`
- `ensure_path_dep_has_version`
- `ensure_workspace_dependency_version`
- `use_workspace_dependency`

## Policy and safety behavior

- `safe` ops run by default when `--apply` is set
- `guarded` ops require `allow_guarded`
- `unsafe` ops require `allow_unsafe`
- Missing params and precondition mismatches block apply results

This crate is internal to the workspace (`publish = false`).
