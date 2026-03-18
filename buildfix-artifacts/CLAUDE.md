# CLAUDE.md

Artifact rendering and file writing for buildfix pipeline outputs.

## Build & Test

```bash
cargo build -p buildfix-artifacts
cargo test -p buildfix-artifacts
```

## Description

Emits plan/apply artifacts (JSON, Markdown, diff) to the filesystem. Uses a pluggable `ArtifactWriter` trait to support both filesystem and in-memory outputs.

## Key Types

- `ArtifactWriter` — trait for writing files/directories
- `FsArtifactWriter` — filesystem implementation
- `write_plan_artifacts()` — emits plan.json, plan.md, comment.md, patch.diff, report.json
- `write_apply_artifacts()` — emits apply.json, apply.md, patch.diff, report.json

## Special Considerations

- Uses `buildfix-render` for Markdown generation
- Creates parent directories automatically
- Both plan and apply outputs include an `extras/` subdirectory with schema-versioned JSON
