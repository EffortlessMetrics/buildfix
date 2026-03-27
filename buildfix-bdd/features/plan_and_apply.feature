Feature: Plan and apply

  Scenario: Adds workspace resolver v2
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml sets workspace resolver to "2"

  Scenario: Adds version to path dependency
    Given a repo with a path dependency missing version
    And a depguard receipt for missing path dependency version
    When I run buildfix plan
    Then the plan contains a path dep version fix
    When I run buildfix apply with --apply
    Then the crate-a Cargo.toml has version for crate-b dependency

  Scenario: Converts to workspace dependency
    Given a repo with a duplicate workspace dependency
    And a depguard receipt for workspace inheritance
    When I run buildfix plan
    Then the plan contains a workspace inheritance fix
    When I run buildfix apply with --apply
    Then the crate-a Cargo.toml uses workspace dependency for serde

  Scenario: Consolidates duplicate dependency versions
    Given a repo with duplicate dependency versions across members
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace dependency serde version "1.0.200"
    And the crate-a Cargo.toml uses workspace dependency for serde
    And the crate-b Cargo.toml uses workspace dependency for serde with feature "derive"

  Scenario: Normalizes MSRV to workspace value
    Given a repo with inconsistent MSRV
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has rust-version "1.70"

  Scenario: Normalizes edition to workspace value
    Given a repo with inconsistent edition
    And a builddiag receipt for edition inconsistency
    When I run buildfix plan
    Then the plan contains an edition normalization fix
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has edition "2021"

  # ============================================================================
  # Error handling and edge cases
  # ============================================================================

  Scenario: Corrupted receipt JSON is skipped gracefully
    Given a repo missing workspace resolver v2
    And a corrupted JSON receipt
    When I run buildfix plan
    Then the plan contains no fixes
    And the report mentions receipt load error

  Scenario: Receipt missing required schema field is skipped gracefully
    Given a repo missing workspace resolver v2
    And a receipt missing the schema field
    When I run buildfix plan
    Then the plan contains no fixes
    And the report mentions receipt load error

  Scenario: Invalid Cargo.toml causes tool error
    Given a repo with malformed Cargo.toml
    And a builddiag receipt for resolver v2
    When I run buildfix plan and capture exit code
    Then the command exits with code 1

  Scenario: Guarded fix skipped without --allow-guarded flag
    Given a repo with inconsistent MSRV
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    When I run buildfix apply with --apply expecting policy block
    Then the crate-a Cargo.toml still has rust-version "1.65"

  Scenario: Dry-run apply does not modify files
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply without --apply
    Then the root Cargo.toml does not have workspace resolver

  Scenario: Empty plan when no matching receipts
    Given a repo missing workspace resolver v2
    And an empty artifacts directory
    When I run buildfix plan
    Then the plan contains no fixes

  Scenario: Apply fails when plan artifact is missing
    Given a repo missing workspace resolver v2
    When I run buildfix apply with --apply expecting missing plan
    Then the command fails with exit code 1
    And the command output mentions "plan.json"

  Scenario: Report capabilities surface partial input failures
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2 with capabilities
    And a corrupted JSON receipt
    When I run buildfix plan
    Then report.json capabilities include check id "workspace.resolver_v2"
    And report.json capabilities include scope "workspace"
    And report.json capabilities mark partial results

  Scenario: Report capabilities are deterministic and sorted
    Given a repo with multiple issues
    And receipts for multiple issues
    When I run buildfix plan
    Then report.json capabilities check ids are sorted
    And report.json capabilities scopes are sorted
    And report.json capabilities inputs available are sorted

  Scenario: Apply report includes deterministic apply metadata
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then report.json apply data field "applied" is 1
    And report.json apply data field "blocked" is 0
    And report.json apply data field "attempted" is 1

  # ============================================================================
  # Policy enforcement
  # ============================================================================

  Scenario: Max ops cap blocks excess ops
    Given a repo with multiple issues
    And receipts for multiple issues
    When I run buildfix plan with --max-ops 1
    Then some plan ops are blocked with reason containing "max_ops"

  Scenario: Allowlist blocks unmatched fixes
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan with allowlist "depguard/*"
    Then the resolver v2 op is blocked by allowlist

  Scenario: Denylist blocks resolver v2
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan with denylist "builddiag/*"
    Then the resolver v2 op is blocked by denylist

  Scenario: Max files cap blocks all ops
    Given a repo with multiple issues
    And receipts for multiple issues
    When I run buildfix plan with --max-files 1
    Then all plan ops are blocked with reason containing "max_files"
    And the patch diff is empty

  Scenario: Max patch bytes cap blocks ops
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan with --max-patch-bytes 1
    Then all plan ops are blocked with reason containing "max_patch_bytes"
    And the patch diff is empty
    And the plan summary patch_bytes is 0

  Scenario: Unsafe fix blocked without params
    Given a repo with a path dependency missing version and no target version
    And a depguard receipt for missing path dependency version
    When I run buildfix plan expecting policy block
    Then the path dependency version op is blocked for missing params

  Scenario: Unused dependency removal requires unsafe opt-in
    Given a repo with an unused dependency
    And a cargo-machete receipt for unused dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix
    When I run buildfix apply with --apply expecting policy block
    Then the crate-a Cargo.toml still has dependency "serde"
    When I run buildfix apply with --apply --allow-unsafe
    Then the crate-a Cargo.toml no longer contains dependency "serde"

  Scenario: Unsafe fix requires --allow-unsafe even with params
    Given a repo with a path dependency missing version and no target version
    And a depguard receipt for missing path dependency version
    When I run buildfix plan with param version "0.3.0"
    Then the plan contains a path dep version fix
    When I run buildfix apply with --apply expecting policy block
    Then the apply results show unsafe fix blocked by safety gate
    When I run buildfix apply with --apply --allow-unsafe
    Then the crate-a Cargo.toml has version "0.3.0" for crate-b dependency

  Scenario: Dirty working tree blocks apply unless allowed
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    And the repo is a clean git repo
    When I run buildfix plan
    And I dirty the working tree
    When I run buildfix apply with --apply expecting policy block
    Then the apply preconditions include dirty working tree mismatch
    When I run buildfix apply with --apply --allow-dirty
    Then the root Cargo.toml sets workspace resolver to "2"

  Scenario: Precondition mismatch aborts apply
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I modify the root Cargo.toml after planning
    When I run buildfix apply with --apply expecting policy block
    Then the apply preconditions are not verified

  # ============================================================================
  # License normalization (cargo-deny)
  # ============================================================================

  Scenario: License normalization requires guarded opt-in
    Given a repo with missing crate license and workspace canonical license
    And a cargo-deny receipt for missing crate license
    When I run buildfix plan
    Then the plan contains a license normalization fix
    When I run buildfix apply with --apply expecting policy block
    Then the crate-a Cargo.toml has no license field
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has license "MIT OR Apache-2.0"

  Scenario: License normalization falls back to unsafe params when canonical is missing
    Given a repo with missing crate license and no workspace canonical license
    And a cargo-deny receipt for missing crate license
    When I run buildfix plan expecting policy block
    Then the license normalization op is blocked for missing params
    When I run buildfix apply with --apply --allow-unsafe --param license "Apache-2.0"
    Then the crate-a Cargo.toml has license "Apache-2.0"

  # ============================================================================
  # Auto-commit
  # ============================================================================

  Scenario: Auto-commit writes commit metadata after successful apply
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    And the repo is a clean git repo
    When I run buildfix plan
    And the repo is a clean git repo
    And I record git HEAD
    When I run buildfix apply with --apply --auto-commit and commit message "buildfix: apply plan"
    Then the root Cargo.toml sets workspace resolver to "2"
    And apply.json records a successful auto-commit
    And apply.json auto-commit message is "buildfix: apply plan"
    And git HEAD changed

  Scenario: Auto-commit is blocked on dirty working tree
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    And the repo is a clean git repo
    When I run buildfix plan
    And I dirty the working tree
    When I run buildfix apply with --apply --auto-commit expecting policy block
    Then the apply results show auto-commit blocked by dirty tree

  # ============================================================================
  # Non-TOML op kinds
  # ============================================================================

  Scenario: Apply executes json yaml and anchored text operations
    Given a repo with non-toml files
    And a handcrafted plan with json yaml and anchored text ops
    When I run buildfix apply with --apply
    Then config.json has service enabled true and no legacy field
    And config.yaml has service enabled true and no legacy field
    And README.md contains anchored replacement "new line"

  # ============================================================================
  # Multiple fixes and determinism
  # ============================================================================

  Scenario: Multiple fixes on same manifest produce stable output
    Given a repo with multiple issues
    And receipts for multiple issues
    When I run buildfix plan
    Then the plan contains multiple fixes
    And the fixes are sorted deterministically

  # ============================================================================
  # Feature preservation
  # ============================================================================

  Scenario: Workspace inheritance preserves dependency features
    Given a repo with workspace dep that has features
    And a depguard receipt for workspace inheritance with features
    When I run buildfix plan
    Then the plan contains a workspace inheritance fix
    When I run buildfix apply with --apply
    Then the crate-a Cargo.toml has workspace serde with features preserved

  # ============================================================================
  # CLI explain command - additional scenarios
  # ============================================================================

  Scenario: Explain command describes resolver-v2 fix
    When I run buildfix explain resolver-v2
    Then the output contains "Key:"

  Scenario: Explain command describes path-dep-version fix
    When I run buildfix explain path-dep-version
    Then the output contains "Key:"

  Scenario: Explain command describes workspace-inheritance fix
    When I run buildfix explain workspace-inheritance
    Then the output contains "Key:"

  Scenario: Explain command describes msrv-normalize fix
    When I run buildfix explain msrv
    Then the output contains "Key:"

  Scenario: Explain command describes edition-normalize fix
    When I run buildfix explain edition
    Then the output contains "Key:"

  Scenario: Explain command fails with unknown fix key
    When I run buildfix explain unknown-fix-key
    Then the command fails with exit code 1
    And the command output mentions "Unknown fix key"

  # ============================================================================
  # CLI list-fixes command - additional scenarios
  # ============================================================================

  Scenario: List fixes output contains safety classes
    When I run buildfix list-fixes
    Then the output contains "Safe"
    And the output contains "Guarded"
    And the output contains "Unsafe"

  Scenario: List fixes JSON output has all required fields
    When I run buildfix list-fixes --format json
    Then the output is valid JSON

  # ============================================================================
  # CLI validate command - additional scenarios
  # ============================================================================

  Scenario: Validate command succeeds with missing artifacts directory
    Given a repo missing workspace resolver v2
    When I run buildfix validate
    Then the command exits with code 0

  Scenario: Validate command fails with invalid plan.json
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I corrupt the plan.json
    And I run buildfix validate
    Then the command fails with exit code 1
    And the command output mentions "json"

  # ============================================================================
  # Safety class verification
  # ============================================================================

  Scenario: Resolver v2 fix is classified as safe
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    And the resolver v2 fix has safety class "safe"

  Scenario: MSRV normalize fix is classified as guarded
    Given a repo with inconsistent MSRV
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix has safety class "guarded"

  Scenario: Unused dependency removal is classified as unsafe
    Given a repo with an unused dependency
    And a cargo-machete receipt for unused dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix
    And the unused dep removal fix has safety class "unsafe"

  Scenario: Path dependency version fix is classified as safe
    Given a repo with a path dependency missing version
    And a depguard receipt for missing path dependency version
    When I run buildfix plan
    Then the plan contains a path dep version fix
    And the path dep version fix has safety class "safe"

  # ============================================================================
  # max_ops limiting - additional edge cases
  # ============================================================================

  Scenario: max-ops zero blocks all operations
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan with --max-ops 0
    Then all plan ops are blocked with reason containing "max_ops"

  Scenario: Plan succeeds when max-ops exceeds operation count
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan with --max-ops 100
    Then the plan contains a resolver v2 fix

  # ============================================================================
  # Error handling - additional scenarios
  # ============================================================================

  Scenario: Plan with empty artifacts directory succeeds
    Given a repo missing workspace resolver v2
    And an empty artifacts directory
    When I run buildfix plan
    Then the plan contains no fixes
    And buildfix validate succeeds

  # ============================================================================
  # CLI help and version
  # ============================================================================

  Scenario: Help output shows available commands
    When I run buildfix with --help
    Then the output contains "plan"
    And the output contains "apply"
    And the output contains "explain"
    And the output contains "list-fixes"
    And the output contains "validate"

  # ============================================================================
  # Complex multi-step scenarios
  # ============================================================================

  Scenario: Multiple fixes with mixed safety classes
    Given a repo with multiple issues
    And receipts for multiple issues
    When I run buildfix plan
    Then the plan contains multiple fixes
    And at least one fix has safety class "safe"

  # ============================================================================
  # Edge cases and robustness
  # ============================================================================

  Scenario: Plan handles receipt with no findings
    Given a repo missing workspace resolver v2
    And a builddiag receipt with no findings
    When I run buildfix plan
    Then the plan contains no fixes

  Scenario: Plan handles receipt with only warnings
    Given a repo missing workspace resolver v2
    And a builddiag receipt with only warnings
    When I run buildfix plan
    Then the plan contains no fixes

  # ============================================================================
  # CLI list-fixes command
  # ============================================================================

  Scenario: List fixes shows available fixes
    When I run buildfix list-fixes
    Then the output contains "resolver-v2"
    And the output contains "Safe"

  Scenario: List fixes supports JSON output
    When I run buildfix list-fixes --format json
    Then the output is valid JSON
    And the JSON output contains fix with key "resolver-v2"
    And the JSON fix output matches enabled builtins

  # ============================================================================
  # Idempotency
  # ============================================================================

  Scenario: Re-running apply on fixed repo produces no changes
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the root Cargo.toml sets workspace resolver to "2"
    When I regenerate receipts for the fixed repo
    And I run buildfix plan
    Then the plan contains no fixes

  Scenario: Running plan twice produces identical output
    Given a repo with multiple issues
    And receipts for multiple issues
    When I run buildfix plan
    And I save the plan.json content
    When I run buildfix plan
    Then the plan.json content is identical to saved

  Scenario: Resolver v2 fixer is idempotent when resolver already exists
    Given a repo with workspace resolver v2 already set
    And a stale builddiag receipt for resolver v2
    When I run buildfix plan
    Then the plan contains no resolver v2 fix

  Scenario: Workspace inheritance fixer is idempotent when already using workspace
    Given a repo with dependency already using workspace inheritance
    And a stale depguard receipt for workspace inheritance
    When I run buildfix plan
    Then the plan contains no workspace inheritance fix

  # ============================================================================
  # Dev-dependencies handling
  # ============================================================================

  Scenario: Converts dev-dependency to workspace inheritance
    Given a repo with duplicate dev-dependency
    And a depguard receipt for dev-dependency inheritance
    When I run buildfix plan
    Then the plan contains a workspace inheritance fix
    When I run buildfix apply with --apply
    Then the crate-a Cargo.toml uses workspace dev-dependency for tokio

  # ============================================================================
  # Artifact validation
  # ============================================================================

  Scenario: Plan produces valid JSON artifacts
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    Then the artifacts directory contains plan.json
    And the artifacts directory contains plan.md
    And the artifacts directory contains patch.diff
    And the artifacts directory contains report.json
    And the plan.json has valid schema version
    And buildfix validate succeeds

  Scenario: Apply produces valid JSON artifacts
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the artifacts directory contains apply.json
    And the artifacts directory contains apply.md
    And the apply.json has valid schema version
    And buildfix validate succeeds

  # ============================================================================
  # Exit code contracts (v0.2.1 operational hardening)
  # ============================================================================

  Scenario: Exit code 0 on successful plan
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan and capture exit code
    Then the command exits with code 0

  Scenario: Exit code 0 on successful apply with --apply
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply with --apply and capture exit code
    Then the command exits with code 0
    And the root Cargo.toml sets workspace resolver to "2"

  Scenario: Exit code 0 on successful dry-run apply without --apply flag
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply without --apply and capture exit code
    Then the command exits with code 0
    And the root Cargo.toml does not have workspace resolver

  Scenario: Exit code 1 for missing plan file
    Given a repo missing workspace resolver v2
    When I run buildfix apply with --apply and capture exit code
    Then the command exits with code 1
    And the command output mentions "plan.json"

  # ============================================================================
  # Evidence-based safety promotion (v0.4.0)
  # ============================================================================

  Scenario: Remove unused deps promoted to Guarded with full evidence
    Given a workspace with an unused dependency "old-crate"
    And a receipt from cargo-machete with high confidence evidence:
      | field         | value |
      | confidence    | 0.95  |
      | analysisDepth | full  |
      | toolAgreement | true  |
    When I run buildfix plan
    Then the plan should contain an operation to remove "old-crate"
    And the operation should have safety class "guarded"

  Scenario: Remove unused deps remains Unsafe with low confidence
    Given a workspace with an unused dependency "old-crate"
    And a receipt from cargo-machete with low confidence evidence:
      | field         | value   |
      | confidence    | 0.7     |
      | analysisDepth | shallow |
      | toolAgreement | false   |
    When I run buildfix plan
    Then the plan should contain an operation to remove "old-crate"
    And the operation should have safety class "unsafe"

  Scenario: Remove unused deps remains Unsafe with missing evidence
    Given a workspace with an unused dependency "old-crate"
    And a receipt from cargo-machete without evidence fields
    When I run buildfix plan
    Then the plan should contain an operation to remove "old-crate"
    And the operation should have safety class "unsafe"

  Scenario: Remove unused deps remains Unsafe with partial evidence
    Given a workspace with an unused dependency "old-crate"
    And a receipt from cargo-machete with partial evidence:
      | field         | value |
      | confidence    | 0.95  |
      | analysisDepth | full  |
      | toolAgreement | false |
    When I run buildfix plan
    Then the plan should contain an operation to remove "old-crate"
    And the operation should have safety class "unsafe"
