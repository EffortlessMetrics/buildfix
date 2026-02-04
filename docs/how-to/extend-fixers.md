# How to Add a New Fixer

This guide walks through extending buildfix with a custom fixer.

## Overview

Fixers are the core planning units in buildfix. Each fixer:

1. Declares metadata (fix key, safety class, consumed sensors/checks)
2. Matches sensor findings via `ReceiptSet`
3. Emits `PlanOp` operations with a safety class

## Architecture

Fixers live in `buildfix-domain/src/fixers/`:

```
buildfix-domain/src/fixers/
├── mod.rs                    # Fixer trait + registry
├── resolver_v2.rs            # Workspace resolver v2
├── path_dep_version.rs       # Path dependency version
├── workspace_inheritance.rs  # Workspace dependency inheritance
└── msrv.rs                   # MSRV normalization
```

## Step 1: Define Metadata

Choose identifiers for your fixer:

- **fix_key**: Unique key like `mysensor.my_fix`
- **sensors**: Which sensor receipts to consume (e.g., `["builddiag", "depguard"]`)
- **check_ids**: Which check IDs to match (e.g., `["my.check_id"]`)

## Step 2: Create the Fixer Module

Create a new file in `buildfix-domain/src/fixers/`:

```rust
// buildfix-domain/src/fixers/my_fixer.rs

use crate::fixers::{Fixer, FixerMeta};
use crate::planner::ReceiptSet;
use crate::ports::RepoView;
use buildfix_types::ops::{OpKind, OpTarget, SafetyClass};
use buildfix_types::plan::{PlanOp, Rationale};
use camino::Utf8PathBuf;
use toml_edit::DocumentMut;

pub struct MyFixer;

impl MyFixer {
    const FIX_ID: &'static str = "mysensor.my_fix";
    const DESCRIPTION: &'static str = "Brief description of what this fix does";
    const SENSORS: &'static [&'static str] = &["mysensor"];
    const CHECK_IDS: &'static [&'static str] = &["my.check_id"];

    fn needs_fix(repo: &dyn RepoView, manifest: &Utf8PathBuf) -> bool {
        let contents = match repo.read_to_string(manifest) {
            Ok(c) => c,
            Err(_) => return false,
        };

        let doc = match contents.parse::<DocumentMut>() {
            Ok(d) => d,
            Err(_) => return false,
        };

        // Determine if fix is needed based on current state
        // Return true if the fix should be applied
        let _ = doc;
        todo!()
    }
}

impl Fixer for MyFixer {
    fn meta(&self) -> FixerMeta {
        FixerMeta {
            fix_key: Self::FIX_ID,
            description: Self::DESCRIPTION,
            safety: SafetyClass::Safe,
            consumes_sensors: Self::SENSORS,
            consumes_check_ids: Self::CHECK_IDS,
        }
    }

    fn plan(
        &self,
        _ctx: &crate::planner::PlanContext,
        repo: &dyn RepoView,
        receipts: &ReceiptSet,
    ) -> anyhow::Result<Vec<PlanOp>> {
        let findings = receipts.matching_findings(Self::SENSORS, Self::CHECK_IDS, &[]);
        if findings.is_empty() {
            return Ok(vec![]);
        }

        let manifest: Utf8PathBuf = "Cargo.toml".into();
        if !Self::needs_fix(repo, &manifest) {
            return Ok(vec![]);
        }

        let fix_key = findings
            .first()
            .map(|f| format!("{}/{}/{}", f.source, f.check_id.clone().unwrap_or_default(), f.code))
            .unwrap_or_else(|| "unknown/-/-".to_string());

        Ok(vec![PlanOp {
            id: String::new(),
            safety: SafetyClass::Safe,
            blocked: false,
            blocked_reason: None,
            target: OpTarget {
                path: manifest.to_string(),
            },
            kind: OpKind::TomlTransform {
                rule_id: "my_rule_id".to_string(),
                args: Some(serde_json::json!({ "example": "value" })),
            },
            rationale: Rationale {
                fix_key,
                description: Some(Self::DESCRIPTION.to_string()),
                findings,
            },
            params_required: vec![],
            preview: None,
        }])
    }
}
```

## Step 3: Define the Operation

Use existing `OpKind` variants when possible:

- `toml_set` for setting a specific TOML path
- `toml_remove` for deleting a TOML path
- `toml_transform` with a new `rule_id` for more complex edits

If you add a new `rule_id`, it must be implemented in the edit engine.

## Step 4: Implement the Edit

Add edit logic to `buildfix-edit/src/lib.rs` in `apply_op_to_content`:

```rust
OpKind::TomlTransform { rule_id, args } => match rule_id.as_str() {
    "my_rule_id" => {
        // Parse and modify using toml_edit
        // doc["section"]["key"] = toml_edit::value("new");
        let _ = args;
    }
    _ => {}
}
```

## Step 5: Register the Fixer

Add to `buildfix-domain/src/fixers/mod.rs`:

```rust
mod my_fixer;

pub fn builtin_fixers() -> Vec<Box<dyn Fixer>> {
    vec![
        Box::new(resolver_v2::ResolverV2Fixer),
        Box::new(path_dep_version::PathDepVersionFixer),
        Box::new(workspace_inheritance::WorkspaceInheritanceFixer),
        Box::new(msrv::MsrvNormalizeFixer),
        Box::new(my_fixer::MyFixer),
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
            check_id: "my.check_id",
            code: None,
        },
    ],
}
```

## Step 7: Write Tests

### BDD Scenario

Add to `buildfix-bdd/features/plan_and_apply.feature`:

```gherkin
Scenario: My fixer applies when finding present
  Given a workspace with my issue
  And a receipt from mysensor with finding my.check_id
  When I run buildfix plan
  Then the plan contains my_rule_id
  And the patch shows the expected change
```

### Unit Test

```rust
#[test]
fn test_my_fixer_produces_op() {
    let fixer = MyFixer;
    let repo = MockRepoView::new(/* setup */);
    let receipts = ReceiptSet::from(/* test receipts */);
    let ctx = PlanContext::default();

    let ops = fixer.plan(&ctx, &repo, &receipts).unwrap();

    assert_eq!(ops.len(), 1);
    match &ops[0].kind {
        OpKind::TomlTransform { rule_id, .. } => assert_eq!(rule_id, "my_rule_id"),
        _ => panic!("expected toml_transform op"),
    }
}
```

## Safety Guidelines

When choosing a safety class:

| Class | Use when |
|-------|----------|
| **Safe** | Single correct answer derivable from repo truth |
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
