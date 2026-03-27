# Support Matrix

This page makes the current support boundary explicit.

`buildfix` is a receipt-driven repair tool. The supported lane is the safe,
deterministic path for workspace hygiene. Operator-reviewed fixes are
deterministic but higher impact. Experimental fixes are still documented and
available, but they are not the default automation path.

## Lanes

| Lane | Status | Fixes / Sensors | When to Use |
|------|--------|-----------------|-------------|
| Supported | Safe | `resolver-v2`, `path-dep-version`, `workspace-inheritance`, `duplicate-deps` driven by `builddiag` and `depguard` receipts | Use for unattended CI and routine workspace hygiene. These are the fixes you should expect to work as the blessed path. |
| Operator-reviewed | Guarded | `msrv`, `edition`, `license` | Use when the change is deterministic but has release or compatibility impact. Review the plan before applying. |
| Experimental | Unsafe | `remove-unused-deps` from `cargo-machete` or `cargo-udeps` | Use only when a human has reviewed the context and is willing to confirm the removal manually. |

## What The Lanes Mean

- Supported means the change is deterministic, derived from repo truth, and
  intended for routine automation.
- Operator-reviewed means the change is deterministic but should not be treated
  as unattended default behavior.
- Experimental means the change exists, but it should be treated as a manual
  decision rather than a promise of safe automation.

## Practical Boundary

If you are writing docs, examples, or CI around the current release boundary,
center the safe `depguard` and `builddiag` lane first. Mention guarded and
unsafe fixes explicitly, but do not describe them as the default operator
experience.

## Related References

- [CLI Reference](cli.md)
- [Fix Catalog](fixes.md)
- [Troubleshooting](../how-to/troubleshoot.md)
