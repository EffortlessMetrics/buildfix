# CLAUDE.md

Embeddable plan/apply pipeline for buildfix (clap-free, port-driven).

## Build & Test

```bash
cargo build -p buildfix-core
cargo test -p buildfix-core
```

## Description

Clap-free, I/O-abstracted core library suitable for embedding into a cockpit mega-binary or other host process. Provides the full plan/apply workflow without CLI dependencies.

## Key Types

- `RepoView` — trait for repository access (from buildfix-domain)
- `run_plan()` — generate a plan + report
- `run_apply()` — apply an existing plan + report

## Port Traits (in `ports` module)

- `ReceiptSource` — load sensor receipts
- `GitPort` — query git state
- `WritePort` — write files and create directories

## Features

- `reporting` — enables buildfix-report
- `artifact-writer` — enables buildfix-artifacts

## Special Considerations

- Re-exports `buildfix_domain::RepoView` and `builtin_fixer_metas` for convenience
- Re-exports receipt types so embedders don't need buildfix-receipts directly
