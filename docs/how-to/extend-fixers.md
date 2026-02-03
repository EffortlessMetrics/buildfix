# How to Add a New Fixer

This guide walks through extending buildfix with a custom fixer.

## Overview

Fixers are the core planning units in buildfix. Each fixer:

1. Matches sensor findings by fix key
2. Probes the repo to determine applicability
3. Emits `PlannedFix` operations with a safety class

## Architecture

Fixers live in `buildfix-domain/src/fixers/`:

```
buildfix-domain/src/fixers/
├── mod.rs                    # Fixer trait + registry
├── resolver_v2.rs            # Workspace resolver v2
├── path_dep_version.rs       # Path dependency version
├── workspace_inheritance.rs  # Workspace dependency inheritance
└── msrv_normalize.rs         # MSRV normalization
```

## Step 1: Define the Fix Key

Choose a fix key pattern that matches sensor findings:

```
sensor_id / check_id / code
```

Example: `depguard / deps.path_requires_version / missing_version`

Add the fix key to your sensor's receipt output.

## Step 2: Create the Fixer Module

Create a new file in `buildfix-domain/src/fixers/`:

```rust
// buildfix-domain/src/fixers/my_fixer.rs

use crate::{Fixer, PlanContext, PlannedFix, RepoView};
use buildfix_receipts::LoadedReceipt;
use buildfix_types::ops::{Operation, SafetyClass};
use buildfix_types::plan::TriggerKey;

pub struct MyFixer;

impl Fixer for MyFixer {
    fn fix_keys(&self) -> &[&str] {
        &["mysensor/mycheck/mycode"]
    }

    fn plan(
        &self,
        ctx: &PlanContext,
        repo: &dyn RepoView,
        receipts: &[LoadedReceipt],
    ) -> Vec<PlannedFix> {
        let mut fixes = Vec::new();

        // Find relevant findings
        for receipt in receipts {
            for finding in &receipt.findings {
                if !self.matches_finding(finding) {
                    continue;
                }

                // Extract info from finding
                let path = match &finding.location {
                    Some(loc) => loc.path.clone(),
                    None => continue,
                };

                // Read repo state
                let content = match repo.read_file(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                // Determine the fix
                let operation = self.determine_operation(&content);
                let safety = self.determine_safety(&content);

                fixes.push(PlannedFix {
                    fix_id: "mysensor.my_fix".to_string(),
                    trigger: TriggerKey {
                        sensor: receipt.tool_name.clone(),
                        check_id: finding.check_id.clone(),
                        code: finding.code.clone(),
                    },
                    target_file: path,
                    operation,
                    safety,
                    rationale: "Explanation of why this fix is needed".to_string(),
                    blocked: false,
                    block_reason: None,
                });
            }
        }

        fixes
    }
}

impl MyFixer {
    fn matches_finding(&self, finding: &buildfix_receipts::Finding) -> bool {
        // Match by check_id and/or code
        finding.check_id.as_deref() == Some("mycheck")
            && finding.code.as_deref() == Some("mycode")
    }

    fn determine_operation(&self, content: &str) -> Operation {
        // Return the appropriate operation variant
        // See buildfix_types::ops::Operation for available variants
        todo!()
    }

    fn determine_safety(&self, content: &str) -> SafetyClass {
        // Most fixes are Safe
        // Use Guarded for high-impact deterministic changes
        // Use Unsafe only when user input is required
        SafetyClass::Safe
    }
}
```

## Step 3: Define the Operation

Add your operation to `buildfix-types/src/ops.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum Operation {
    // ... existing variants ...

    /// My new operation
    MyOperation {
        /// Target file path
        file: String,
        /// Value to set
        value: String,
    },
}
```

## Step 4: Implement the Edit

Add edit logic to `buildfix-edit/src/lib.rs`:

```rust
fn apply_operation(
    repo_root: &Utf8Path,
    op: &Operation,
    dry_run: bool,
) -> Result<String> {
    match op {
        // ... existing cases ...

        Operation::MyOperation { file, value } => {
            let path = repo_root.join(file);
            let content = fs::read_to_string(&path)?;

            // Parse and modify
            let mut doc = content.parse::<DocumentMut>()?;
            // ... make changes ...

            let new_content = doc.to_string();

            if !dry_run {
                fs::write(&path, &new_content)?;
            }

            Ok(new_content)
        }
    }
}
```

## Step 5: Register the Fixer

Add to the fixer registry in `buildfix-domain/src/fixers/mod.rs`:

```rust
mod my_fixer;

pub use my_fixer::MyFixer;

pub fn all_fixers() -> Vec<Box<dyn Fixer>> {
    vec![
        Box::new(ResolverV2Fixer),
        Box::new(PathDepVersionFixer),
        Box::new(WorkspaceInheritanceFixer),
        Box::new(MsrvNormalizeFixer),
        Box::new(MyFixer),  // Add here
    ]
}
```

## Step 6: Add Explanation

Add to `buildfix-cli/src/explain.rs`:

```rust
FixExplanation {
    key: "my-fix",
    fix_id: "mysensor.my_fix",
    title: "My Fix",
    safety: SafetyClass::Safe,
    description: r#"What this fix does..."#,
    safety_rationale: r#"Why it's safe..."#,
    remediation: r#"How to fix manually..."#,
    triggers: &[
        TriggerPattern {
            sensor: "mysensor",
            check_id: "mycheck",
            code: Some("mycode"),
        },
    ],
},
```

## Step 7: Write Tests

### BDD Scenario

Create `buildfix-bdd/tests/features/my_fixer.feature`:

```gherkin
Feature: My fixer

  Scenario: Fix is applied when finding present
    Given a workspace with my issue
    And a receipt from mysensor with finding mycheck/mycode
    When I run buildfix plan
    Then the plan contains my_fix
    And the patch shows the expected change

  Scenario: Fix is skipped when not applicable
    Given a workspace without my issue
    And a receipt from mysensor with finding mycheck/mycode
    When I run buildfix plan
    Then the plan is empty
```

### Golden Fixture

Create `buildfix-domain/tests/fixtures/my-fixer/`:

```
my-fixer/
├── input/
│   ├── Cargo.toml
│   └── artifacts/mysensor/report.json
├── expected/
│   ├── plan.json
│   └── patch.diff
└── README.md
```

### Unit Test

```rust
#[test]
fn test_my_fixer_produces_fix() {
    let fixer = MyFixer;
    let ctx = test_context();
    let repo = MockRepoView::new(/* ... */);
    let receipts = vec![/* ... */];

    let fixes = fixer.plan(&ctx, &repo, &receipts);

    assert_eq!(fixes.len(), 1);
    assert_eq!(fixes[0].fix_id, "mysensor.my_fix");
}
```

## Safety Guidelines

When choosing a safety class:

| Class | Use when |
|-------|----------|
| **Safe** | Single correct answer from repo truth |
| **Guarded** | Deterministic but affects compatibility/workflow |
| **Unsafe** | Multiple valid choices, needs user input |

Key rules:
- Never invent values—derive from repo or require user params
- Produce minimal diffs—only change what's needed
- Be deterministic—same inputs = same outputs
- Make it reversible—backups and clear rationale

## See Also

- [Architecture](../architecture.md)
- [Safety Model](../safety-model.md)
- [Design Goals](../design.md)
