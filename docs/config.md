# buildfix — Configuration

buildfix has two layers of policy:

1) **buildfix.toml** (actuator policy: allow/deny, safety, caps)
2) **cockpit.toml** (composition policy: whether buildfix runs and whether its output blocks)

## buildfix.toml

This config is consumed by buildfix. It determines what can be planned/applied.

### Example (see examples/buildfix.toml)

- allowlist and denylist
- whether guarded ops may apply
- caps on operations/files/diff size
- backup policy
- parameters for unsafe ops (optional)

### Matching rules

Allow/deny entries support:
- exact policy keys: `depguard/deps.path_requires_version/missing_version`
- prefix patterns: `depguard/*`
- code patterns (discouraged; prefer fix keys)

Rules are evaluated in this order:
1) explicit deny wins
2) if allowlist non-empty, only allowlisted keys may proceed
3) otherwise allowlisted-by-default safe ops are eligible

## cockpit.toml

Cockpit policy is separate: buildfix should not hardcode governance.

Typical patterns:
- buildfix runs only in maintainer workflows
- or runs as plan-only in PRs and posts patch as artifact

Recommended default for general repos:
- buildfix is informational (non-blocking), missing receipt is skip.

## Parameters for unsafe ops

Unsafe operations can be unblocked only by supplying parameters, either via CLI:
- `--param rust_version=1.75`
or via config:
- `params.rust_version = "1.75"`

Parameters are recorded in plan/apply artifacts for auditability.
