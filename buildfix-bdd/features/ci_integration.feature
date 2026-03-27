Feature: CI Integration

  This feature covers CI/CD integration scenarios for buildfix, ensuring
  correct behavior in different pipeline configurations.

  # PR Lane: Plan-only mode (no modifications)
  Scenario: PR lane generates plan without applying - exit code 0 on safe fixes
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan and capture exit code
    Then the command exits with code 0
    And the artifacts directory contains plan.json
    And the artifacts directory contains plan.md
    And the artifacts directory contains patch.diff
    And the root Cargo.toml does not have workspace resolver

  Scenario: PR lane generates plan with exit code 2 when policy blocks
    Given a repo with a path dependency missing version and no target version
    And a depguard receipt for missing path dependency version
    When I run buildfix plan and capture exit code
    Then the command exits with code 2
    And the artifacts directory contains plan.json
    And the plan.json contains blocked ops

  Scenario: PR lane dry-run apply shows preview without modifying files
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply without --apply and capture exit code
    Then the command exits with code 0
    And the root Cargo.toml does not have workspace resolver
    And the artifacts directory contains patch.diff

  # Main Lane: Auto-apply safe fixes
  Scenario: Main lane applies safe fixes only
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply with --apply and capture exit code
    Then the command exits with code 0
    And the root Cargo.toml sets workspace resolver to "2"
    And the artifacts directory contains apply.json

  Scenario: Main lane with mixed safe and guarded fixes applies only safe
    Given a repo with multiple issues including guarded
    And receipts for multiple issues including guarded
    When I run buildfix plan
    And I run buildfix apply with --apply and capture exit code
    Then the command exits with code 0
    And the root Cargo.toml sets workspace resolver to "2"
    And the crate-a Cargo.toml still has rust-version "1.65"

  Scenario: Main lane with only guarded fixes exits 2 without --allow-guarded
    Given a repo with inconsistent MSRV
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    And I run buildfix apply with --apply expecting policy block
    Then the crate-a Cargo.toml still has rust-version "1.65"

  # Guarded operations policy
  Scenario: Guarded ops remain blocked without explicit flag
    Given a repo with inconsistent MSRV
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    When I run buildfix apply with --apply expecting policy block
    Then the crate-a Cargo.toml still has rust-version "1.65"
    And the apply results show guarded fix blocked

  Scenario: Guarded ops applied with --allow-guarded flag
    Given a repo with inconsistent MSRV
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    And I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has rust-version "1.70"

  Scenario: Mixed safe and guarded ops with --allow-guarded applies all
    Given a repo with multiple issues including guarded
    And receipts for multiple issues including guarded
    When I run buildfix plan
    And I run buildfix apply with --apply --allow-guarded
    Then the root Cargo.toml sets workspace resolver to "2"
    And the crate-a Cargo.toml has rust-version "1.70"

  # Artifact generation for CI
  Scenario: Plan generates all required CI artifacts
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    Then the artifacts directory contains plan.json
    And the artifacts directory contains plan.md
    And the artifacts directory contains patch.diff
    And the artifacts directory contains report.json
    And the plan.json has valid schema version

  Scenario: Apply generates apply.json artifact
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the artifacts directory contains apply.json
    And the apply.json has valid schema version

  Scenario: Patch diff is valid unified diff format
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    Then the artifacts directory contains patch.diff
    And the patch.diff contains valid diff headers

  Scenario: Plan.md contains human-readable summary
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    Then the artifacts directory contains plan.md
    And the plan.md contains summary section

  # Exit code semantics for CI
  Scenario: Exit code 0 indicates success with no issues
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan and capture exit code
    Then the command exits with code 0

  Scenario: Exit code 1 indicates tool error
    Given a repo with malformed Cargo.toml
    And a builddiag receipt for resolver v2
    When I run buildfix plan and capture exit code
    Then the command exits with code 1

  Scenario: Exit code 2 indicates policy block
    Given a repo with a path dependency missing version and no target version
    And a depguard receipt for missing path dependency version
    When I run buildfix plan and capture exit code
    Then the command exits with code 2

  Scenario: Empty plan exits with code 0
    Given a repo missing workspace resolver v2
    And an empty artifacts directory
    When I run buildfix plan and capture exit code
    Then the command exits with code 0
    And the plan contains no fixes

  Scenario: Apply without plan exits with code 1
    Given a repo missing workspace resolver v2
    When I run buildfix apply with --apply expecting missing plan
    Then the command fails with exit code 1

  # CI failure handling
  Scenario: CI continues on empty plan
    Given a repo missing workspace resolver v2
    And an empty artifacts directory
    When I run buildfix plan and capture exit code
    Then the command exits with code 0
    And the plan contains no fixes

  Scenario: CI can detect if fixes were applied
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then report.json apply data field "applied" is 1
    And report.json apply data field "blocked" is 0

  Scenario: CI can detect blocked fixes
    Given a repo with inconsistent MSRV
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    And I run buildfix apply with --apply expecting policy block
    Then report.json apply data field "applied" is 0
    And report.json apply data field "blocked" is at least 1

  Scenario: Precondition mismatch returns exit code 2
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    And I modify the root Cargo.toml after planning
    And I run buildfix apply with --apply expecting policy block
    Then the apply preconditions are not verified
