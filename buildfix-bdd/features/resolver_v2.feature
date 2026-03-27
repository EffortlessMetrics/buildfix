Feature: Workspace Resolver V2 Normalization

  Background:
    Given a repo with workspace needing resolver v2

  # ============================================================================
  # Safety classification scenarios
  # ============================================================================

  Scenario: Resolver v2 fix is classified as safe
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    And the resolver v2 fix has safety class "safe"

  Scenario: Resolver v2 fix applies automatically with --apply
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace resolver "2"

  Scenario: Resolver v2 fix requires no guarded opt-in
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    And the resolver v2 fix does not require --allow-guarded

  # ============================================================================
  # Adding resolver scenarios
  # ============================================================================

  Scenario: Resolver v2 fix adds missing resolver field to workspace
    Given a builddiag receipt for resolver v2
    And a workspace with no resolver field
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace resolver "2"

  Scenario: Resolver v2 fix adds resolver to virtual workspace
    Given a builddiag receipt for resolver v2
    And a virtual workspace with no resolver field
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace resolver "2"

  # ============================================================================
  # Updating resolver scenarios
  # ============================================================================

  Scenario: Resolver v2 fix updates resolver from "1" to "2"
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace resolver "2"

  Scenario Outline: Resolver v2 fix updates various resolver values to "2"
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "<current>"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace resolver "2"

    Examples:
      | current |
      | 1       |
      | "1"     |

  # ============================================================================
  # No-op scenarios
  # ============================================================================

  Scenario: Resolver v2 fix is no-op when already set to "2"
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "2"
    When I run buildfix plan
    Then the plan contains no resolver v2 fix

  Scenario: Resolver v2 fix is no-op when resolver is already "2" string
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "2"
    When I run buildfix plan
    Then the plan contains no resolver v2 fix

  # ============================================================================
  # Non-workspace scenarios
  # ============================================================================

  Scenario: Resolver v2 fix is not applicable to single package projects
    Given a builddiag receipt for resolver v2
    And a single package project with no workspace
    When I run buildfix plan
    Then the plan contains no resolver v2 fix

  Scenario: Resolver v2 fix skips non-workspace manifests
    Given a builddiag receipt for resolver v2
    And a crate manifest without workspace section
    When I run buildfix plan
    Then the plan contains no resolver v2 fix

  # ============================================================================
  # Virtual workspace vs package workspace scenarios
  # ============================================================================

  Scenario: Resolver v2 fix works with virtual workspace
    Given a builddiag receipt for resolver v2
    And a virtual workspace with members
    And no root package
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace resolver "2"

  Scenario: Resolver v2 fix works with package workspace
    Given a builddiag receipt for resolver v2
    And a workspace with root package and members
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace resolver "2"

  # ============================================================================
  # Idempotent operation scenarios
  # ============================================================================

  Scenario: Resolver v2 fix is idempotent across multiple plan runs
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace resolver "2"
    When I run buildfix plan again
    Then the plan contains no resolver v2 fix

  Scenario: Resolver v2 fix does not duplicate on re-run
    Given a builddiag receipt for resolver v2
    And a workspace with no resolver field
    When I run buildfix plan
    Then the plan contains exactly 1 resolver v2 fix
    When I run buildfix apply with --apply
    And I run buildfix plan again
    Then the plan contains no resolver v2 fix

  # ============================================================================
  # Receipt source scenarios
  # ============================================================================

  Scenario: Resolver v2 fix is triggered by builddiag receipt
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    And the resolver v2 fix has fix key matching "builddiag/workspace.resolver_v2/*"

  Scenario: Resolver v2 fix is triggered by cargo receipt
    Given a cargo receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    And the resolver v2 fix has fix key matching "cargo/workspace.resolver_v2/*"

  Scenario Outline: Resolver v2 fix is triggered by multiple receipt sources
    Given a <source> receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix

    Examples:
      | source     |
      | builddiag  |
      | cargo      |

  Scenario: Resolver v2 fix aggregates findings from multiple receipts
    Given a builddiag receipt for resolver v2
    And a cargo receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    And the resolver v2 fix references all triggering findings

  # ============================================================================
  # Check ID scenarios
  # ============================================================================

  Scenario Outline: Resolver v2 fix responds to various check IDs
    Given a receipt with check_id "<check_id>"
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix

    Examples:
      | check_id                      |
      | workspace.resolver_v2         |
      | cargo.workspace.resolver_v2   |

  # ============================================================================
  # Error handling scenarios
  # ============================================================================

  Scenario: Resolver v2 fix handles missing Cargo.toml gracefully
    Given a builddiag receipt for resolver v2
    And no Cargo.toml file
    When I run buildfix plan
    Then the plan contains no resolver v2 fix

  Scenario: Resolver v2 fix handles invalid TOML gracefully
    Given a builddiag receipt for resolver v2
    And an invalid Cargo.toml file
    When I run buildfix plan
    Then the plan contains no resolver v2 fix

  # ============================================================================
  # Determinism scenarios
  # ============================================================================

  Scenario: Resolver v2 fix produces deterministic output
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix plan again
    Then the plan contains a resolver v2 fix with identical content

  # ============================================================================
  # Rationale scenarios
  # ============================================================================

  Scenario: Resolver v2 fix includes clear rationale
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    And the resolver v2 fix has rationale containing "resolver"
    And the resolver v2 fix has rationale containing "feature unification"

  # ============================================================================
  # Target scenarios
  # ============================================================================

  Scenario: Resolver v2 fix targets root Cargo.toml only
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    And the resolver v2 fix targets path "Cargo.toml"

  Scenario: Resolver v2 fix uses correct TOML transform rule
    Given a builddiag receipt for resolver v2
    And a workspace with resolver "1"
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    And the resolver v2 fix uses rule "ensure_workspace_resolver_v2"
