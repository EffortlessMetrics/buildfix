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

  Scenario: Normalizes MSRV to workspace value
    Given a repo with inconsistent MSRV
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has rust-version "1.70"

  # ============================================================================
  # Error handling and edge cases
  # ============================================================================

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

  # ============================================================================
  # Policy enforcement
  # ============================================================================

  Scenario: Plan fails when max_ops cap exceeded
    Given a repo with a path dependency missing version
    And a depguard receipt for missing path dependency version
    When I run buildfix plan with --max-ops 0
    Then the plan command fails

  Scenario: Allowlist blocks unmatched fixes
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan with allowlist "depguard/*"
    Then the resolver v2 op is blocked by allowlist

  Scenario: Unsafe fix blocked without params
    Given a repo with a path dependency missing version and no target version
    And a depguard receipt for missing path dependency version
    When I run buildfix plan expecting policy block
    Then the path dependency version op is blocked for missing params

  Scenario: Precondition mismatch aborts apply
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I modify the root Cargo.toml after planning
    When I run buildfix apply with --apply expecting policy block
    Then the apply preconditions are not verified

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
  # CLI explain command
  # ============================================================================

  Scenario: Explain command describes a fix
    When I run buildfix explain resolver-v2
    Then the output contains the fix description

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

  Scenario: Apply produces valid JSON artifacts
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the artifacts directory contains apply.json
    And the artifacts directory contains apply.md
    And the apply.json has valid schema version
