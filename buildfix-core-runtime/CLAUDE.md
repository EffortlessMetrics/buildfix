# CLAUDE.md

Core runtime adapters and settings for buildfix embeddings.

## Build & Test

```bash
cargo build -p buildfix-core-runtime
cargo test -p buildfix-core-runtime
```

## Description

Provides port traits and default adapters for I/O operations. Used by buildfix-core for filesystem, git, and receipt loading. Intentionally minimal—just wiring and configuration.

## Key Types

- `ReceiptSource` — trait to load sensor receipts
- `GitPort` — trait for git queries (HEAD SHA, dirty status)
- `WritePort` — trait for file writes and directory creation
- `PlanSettings` / `ApplySettings` — configuration for plan/apply pipelines
- `RunMode` — Standalone or Cockpit (affects exit code semantics)

## Features

- `fs` — filesystem adapters
- `git` — git shell adapter
- `memory` — in-memory receipt source for testing

## Special Considerations

- Default features: fs, git, memory
- In `Cockpit` mode, policy blocks (exit 2) map to exit 0 since the receipt encodes the block
