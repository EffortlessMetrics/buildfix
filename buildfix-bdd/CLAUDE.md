# buildfix-bdd

Behavior-driven testing using cucumber-rs for workflow contracts.

## Build & Test

```bash
cargo test -p buildfix-bdd
```

## Structure

```
buildfix-bdd/
  features/
    plan_and_apply.feature   # Gherkin scenarios
  tests/
    cucumber.rs              # Step definitions and World
```

## Test Scenarios

| Scenario | Description |
|----------|-------------|
| Adds workspace resolver v2 | Creates workspace without resolver, runs plan+apply |
| Adds version to path dependency | Path dep missing version, verifies version added |
| Converts to workspace dependency | Duplicate deps, verifies `workspace = true` |
| Normalizes MSRV to workspace value | Inconsistent MSRV, requires `--allow-guarded` |
| Denylist blocks resolver v2 | Denylist policy blocks plan ops |
| Max files cap blocks all ops | `--max-files` caps block all ops + empty patch |
| Max patch bytes cap blocks ops | `--max-patch-bytes` caps block ops + zero patch |
| Unsafe fix requires allow-unsafe | Params provided, but apply blocks without `--allow-unsafe` |
| Dirty working tree blocks apply | Dirty tree blocks apply unless `--allow-dirty` |

## Implementation Pattern

```rust
#[derive(World)]
struct BuildfixWorld {
    temp_dir: TempDir,
    repo_root: PathBuf,
}

#[given("a workspace with...")]
async fn setup_workspace(world: &mut BuildfixWorld) { ... }

#[when("I run buildfix plan")]
async fn run_plan(world: &mut BuildfixWorld) { ... }

#[then("the plan contains...")]
async fn verify_plan(world: &mut BuildfixWorld) { ... }
```

## Running Tests

```bash
# Run all BDD tests
cargo test -p buildfix-bdd

# Run with cucumber output
cargo test -p buildfix-bdd -- --nocapture
```

## Writing New Scenarios

1. Add scenario to `features/plan_and_apply.feature`
2. Implement missing steps in `tests/cucumber.rs`
3. Use `assert_cmd` to invoke CLI
4. Validate JSON artifacts for correctness

## Dependencies

- `cucumber` - BDD framework
- `assert_cmd` - CLI testing
- `tempfile` - Isolated test directories
- `serde_json` - Artifact validation
