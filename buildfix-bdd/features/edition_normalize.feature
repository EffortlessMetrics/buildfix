Feature: Edition Normalization

  Background:
    Given a repo with inconsistent edition

  # ============================================================================
  # Safety classification scenarios
  # ============================================================================

  Scenario: Edition normalization is classified as guarded
    Given a builddiag receipt for edition inconsistency
    When I run buildfix plan
    Then the plan contains an edition normalization fix
    And the edition fix has safety class "guarded"

  Scenario: Edition normalization requires guarded opt-in
    Given a builddiag receipt for edition inconsistency
    When I run buildfix plan
    Then the plan contains an edition normalization fix
    When I run buildfix apply with --apply expecting policy block
    Then the crate-a Cargo.toml has edition "2018"

  Scenario: Edition normalization applies with --allow-guarded
    Given a builddiag receipt for edition inconsistency
    When I run buildfix plan
    Then the plan contains an edition normalization fix
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has edition "2021"

  # ============================================================================
  # Workspace standard scenarios
  # ============================================================================

  Scenario: Edition normalization uses workspace.package.edition as canonical
    Given a repo with workspace package edition "2021"
    And a crate with edition "2018"
    And a builddiag receipt for edition inconsistency
    When I run buildfix plan
    Then the plan contains an edition normalization fix
    And the edition fix targets edition "2021"

  Scenario: Edition normalization falls back to root package edition
    Given a repo with root package edition "2021" but no workspace package edition
    And a crate with edition "2018"
    And a builddiag receipt for edition inconsistency
    When I run buildfix plan
    Then the plan contains an edition normalization fix
    And the edition fix targets edition "2021"

  # ============================================================================
  # Missing edition scenarios
  # ============================================================================

  Scenario: Edition normalization adds missing edition field
    Given a repo with workspace package edition "2021"
    And a crate with missing edition field
    And a builddiag receipt for missing edition
    When I run buildfix plan
    Then the plan contains an edition normalization fix
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has edition "2021"

  Scenario: Edition normalization blocked when no canonical edition exists
    Given a repo with no canonical edition
    And a crate with edition "2018"
    And a builddiag receipt for edition inconsistency
    When I run buildfix plan expecting policy block
    Then the edition normalization op is blocked for missing params

  # ============================================================================
  # Multiple crates scenarios
  # ============================================================================

  Scenario: Edition normalization handles multiple crates with different editions
    Given a workspace with multiple crates having different editions
    And a builddiag receipt for multiple edition inconsistencies
    When I run buildfix plan
    Then the plan contains multiple edition normalization fixes
    And all edition fixes target the same canonical edition

  Scenario: Edition normalization skips crates already at canonical edition
    Given a repo with workspace package edition "2021"
    And crate-a with edition "2021"
    And crate-b with edition "2018"
    And a builddiag receipt for edition inconsistency for crate-b only
    When I run buildfix plan
    Then the plan contains exactly 1 edition normalization fix
    And the edition fix targets crate-b

  # ============================================================================
  # Determinism scenarios
  # ============================================================================

  Scenario: Edition normalization fixes are sorted deterministically
    Given a workspace with multiple crates having different editions
    And a builddiag receipt for multiple edition inconsistencies
    When I run buildfix plan
    Then the edition fixes are sorted by manifest path

  # ============================================================================
  # Receipt integration scenarios
  # ============================================================================

  Scenario: Edition normalization responds to builddiag sensor
    Given a repo with inconsistent edition
    And a builddiag receipt for edition inconsistency
    When I run buildfix plan
    Then the plan contains an edition normalization fix

  Scenario: Edition normalization responds to cargo sensor check ids
    Given a repo with inconsistent edition
    And a cargo receipt for edition inconsistency
    When I run buildfix plan
    Then the plan contains an edition normalization fix
