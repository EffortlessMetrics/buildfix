# CLAUDE.md

Small hashing primitives used across buildfix internals.

## Build & Test

```bash
cargo build -p buildfix-hash
cargo test -p buildfix-hash
```

## Description

Minimal SHA-256 hashing utilities. Just one function—used by other crates for content-addressing.

## Key Functions

- `sha256_hex()` — returns lowercase hex SHA-256 digest of provided bytes

## Special Considerations

- Uses `sha2` and `hex` crates
- Output is always 64 hex characters (32 bytes)
- Deterministic: same input always produces same output
