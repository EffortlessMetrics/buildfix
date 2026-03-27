# buildfix configuration profiles

These profiles are ready-made `buildfix.toml` starting points. Pick the one that matches how much automation you want to trust on the first run.

## Choose a profile

| Profile | Use when | Safe ops | Guarded ops | Unsafe ops |
|---------|----------|----------|-------------|------------|
| [`conservative`](#conservative) | CI, bots, or any run that should only make low-risk edits | Auto | Blocked | Blocked |
| [`balanced`](#balanced) | Human-reviewed maintenance runs | Auto | Allowed with `--allow-guarded` | Blocked |
| [`aggressive-but-reviewed`](#aggressive-but-reviewed) | Planned cleanup on a repo you understand well | Auto | Allowed with `--allow-guarded` | Allowed with `--allow-unsafe` |

If you are not sure, start with `balanced`. If the run is unattended, start with `conservative`.

## First use

1. Copy the profile into the repository root as `buildfix.toml`.
2. Run `cargo run -p buildfix -- plan` and review `plan.md` plus `patch.diff`.
3. Apply only the safety class you intended.

```bash
cp examples/profiles/<profile-name>.toml buildfix.toml
cargo run -p buildfix -- plan
cargo run -p buildfix -- apply --apply
```

## What the safety labels mean

- `safe`: buildfix can derive the edit from repository truth
- `guarded`: deterministic, but the change is broad enough to warrant review
- `unsafe`: buildfix needs an explicit operator choice or parameter

## conservative

Use this for unattended automation and CI.

This profile only permits the safe lane:

- workspace resolver normalization
- path dependency version insertion
- workspace dependency inheritance
- duplicate dependency consolidation

It blocks guarded and unsafe operations, so it is the safest default for a new repository.

```bash
cp examples/profiles/conservative.toml buildfix.toml
cargo run -p buildfix -- plan
cargo run -p buildfix -- apply --apply
```

## balanced

Use this when a person will review the plan before merge.

This profile still auto-applies safe operations, but it opens the guarded lane for:

- MSRV normalization
- edition normalization
- license normalization

It does not allow unsafe fixes. Use this when you want a useful default without giving the tool too much freedom.

```bash
cp examples/profiles/balanced.toml buildfix.toml
cargo run -p buildfix -- plan
cargo run -p buildfix -- apply --apply --allow-guarded
```

## aggressive-but-reviewed

Use this for deliberate cleanup sessions on a repo you trust.

This profile allows everything the current catalog supports, including unsafe dependency removal. That means you must review the plan carefully before apply.

```bash
cp examples/profiles/aggressive-but-reviewed.toml buildfix.toml
cargo run -p buildfix -- plan
cargo run -p buildfix -- apply --apply --allow-guarded
cargo run -p buildfix -- apply --apply --allow-guarded --allow-unsafe
```

## Practical guidance

- Start with `conservative` for bots and release automation.
- Start with `balanced` for an operator-led first run.
- Use `aggressive-but-reviewed` only when you already know which fixes you want and have checked the plan output.
- If a run touches MSRV, edition, license, or dependency removal, read the diff before you apply it.

## Further reading

- [Support matrix](../../docs/reference/support-matrix.md)
- [Configuration guide](../../docs/how-to/configure.md)
- [Configuration schema](../../docs/config.md)
- [Fix catalog](../../docs/reference/fixes.md)
