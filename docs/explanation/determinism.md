# Determinism: Why Byte-Stable Outputs Matter

This document explains buildfix's determinism guarantees and why they're essential.

## The Guarantee

Given the same inputs:
- Same receipts
- Same repo state
- Same config

buildfix produces **byte-identical** outputs:
- Same `plan.json`
- Same `patch.diff`
- Same ordering of operations

## Why Determinism Matters

### 1. Reproducible Builds

When buildfix is deterministic:
- Running plan twice produces the same result
- CI builds are reproducible
- Debugging is possible by replaying inputs

Without determinism:
- Flaky CI runs
- "Works on my machine" problems
- Impossible to reproduce reported issues

### 2. Code Review

Deterministic diffs enable meaningful review:
- Reviewers can trust the diff represents actual changes
- No spurious changes from reordering
- Patch preview matches what will be applied

Without determinism:
- Random reordering creates noise
- Reviewers can't trust diffs
- Hard to spot real changes among churn

### 3. Incremental Adoption

Teams can adopt buildfix gradually:
- Run plan repeatedly with confidence
- Compare plans across branches
- Detect when findings change

Without determinism:
- Plans differ randomly
- Hard to tell if findings changed or just ordering
- Difficult to track progress

### 4. Audit Trail

Deterministic outputs support auditing:
- Plans can be verified against repo state
- Changes are attributable to specific findings
- Compliance requirements are satisfiable

Without determinism:
- Audit logs are unreliable
- Hard to prove what changed and why
- Compliance becomes difficult

## How buildfix Achieves Determinism

### 1. Stable Sorting

All collections are sorted deterministically:

```rust
// Fixes sorted by stable key
fixes.sort_by_key(|f| stable_fix_sort_key(f));

// Files in consistent order
files.sort_by(|a, b| a.path.cmp(&b.path));
```

The sort key includes:
- Target file path
- Fix ID
- Operation type

This ensures the same ordering regardless of:
- Receipt discovery order
- HashMap iteration order
- Thread scheduling

### 2. Deterministic IDs

Plan IDs use stable inputs:

```rust
// NOT: Uuid::new_v4() (random)
// Instead: derived from content hash
```

In practice, buildfix uses UUID v4 for `plan_id` because:
- Each plan is genuinely unique
- But the plan *content* is deterministic
- The ID is just a correlation handle

### 3. No Timing Dependencies

buildfix avoids:
- Random number generators
- Wall clock time in content (only metadata)
- Thread-local state
- Order-dependent HashMap iteration

### 4. Normalized Paths

All paths are normalized:
- Repo-relative
- Forward slashes (even on Windows)
- No leading `./`
- No trailing slashes

```rust
fn normalize_path(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
```

### 5. Consistent JSON Serialization

JSON output uses:
- Sorted keys
- Consistent indentation
- No trailing whitespace variation

```rust
serde_json::to_string_pretty(&plan)?
```

## Testing Determinism

### Golden Fixtures

Test fixtures capture expected outputs:

```
tests/fixtures/resolver-v2/
├── input/
│   ├── Cargo.toml
│   └── artifacts/builddiag/report.json
└── expected/
    ├── plan.json
    └── patch.diff
```

Tests verify byte-exact match:

```rust
#[test]
fn test_resolver_v2_determinism() {
    let actual = run_plan("resolver-v2");
    let expected = read_fixture("resolver-v2/expected/plan.json");
    assert_eq!(actual, expected);
}
```

### Property Testing

Proptest verifies invariants:

```rust
proptest! {
    #[test]
    fn plan_is_deterministic(receipts: Vec<Receipt>) {
        let plan1 = generate_plan(&receipts);
        let plan2 = generate_plan(&receipts);
        assert_eq!(plan1, plan2);
    }
}
```

### Multiple Runs

CI runs plan multiple times and compares:

```bash
buildfix plan --out-dir /tmp/plan1
buildfix plan --out-dir /tmp/plan2
diff /tmp/plan1/plan.json /tmp/plan2/plan.json
```

## Acceptable Non-Determinism

Some fields are intentionally non-deterministic:

| Field | Reason |
|-------|--------|
| `plan_id` | Correlation handle, not content |
| `created_at` | Metadata timestamp |
| `applied_at` | Metadata timestamp |
| `tool.commit` | Build-time metadata |

These don't affect:
- What changes are planned
- How changes are applied
- The patch content

## When Determinism Appears Broken

If you see different outputs, check:

### Different Inputs

```bash
# Are receipts identical?
diff artifacts1/ artifacts2/

# Are repo files identical?
git diff

# Is config identical?
diff buildfix1.toml buildfix2.toml
```

### Version Differences

```bash
# Same buildfix version?
buildfix --version
```

Different versions may:
- Sort differently
- Produce different fix IDs
- Have different default behavior

### Floating Timestamps

Timestamps in metadata are expected to differ:

```json
{"created_at": "2024-01-15T10:30:00Z"}  // Plan 1
{"created_at": "2024-01-15T10:31:00Z"}  // Plan 2
```

Compare semantic content, not metadata:

```bash
jq 'del(.created_at, .plan_id)' plan1.json > norm1.json
jq 'del(.created_at, .plan_id)' plan2.json > norm2.json
diff norm1.json norm2.json
```

## See Also

- [Preconditions](preconditions.md)
- [Design Goals](../design.md)
- [Testing Strategy](../testing.md)
