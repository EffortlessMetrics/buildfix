# buildfix fuzzing

This is a cargo-fuzz compatible folder (not included as a workspace member).

Example:

```bash
cargo install cargo-fuzz
cargo fuzz run apply_op
```

Targets are intentionally small and focused on parser resilience + idempotence.
