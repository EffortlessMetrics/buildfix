Feature: Duplicate Dependencies Consolidation

  # ============================================================================
  # Safety classification scenarios
  # ============================================================================

  Scenario: Duplicate dependency consolidation is classified as safe
    Given a repo with duplicate dependency versions across members
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    And the duplicate deps fix has safety class "safe"

  Scenario: Duplicate dependency consolidation applies without --allow-guarded
    Given a repo with duplicate dependency versions across members
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml has workspace dependency serde version "1.0.200"
    And the crate-a Cargo.toml uses workspace dependency for serde
    And the crate-b Cargo.toml uses workspace dependency for serde with feature "derive"

  # ============================================================================
  # Workspace dependency creation scenarios
  # ============================================================================

  Scenario: Consolidation creates workspace.dependencies entry
    Given a repo with duplicate dependency versions across members
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    And the plan contains a root op to add workspace dependency

  Scenario: Consolidation converts member deps to workspace = true
    Given a repo with duplicate dependency versions across members
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the plan contains member ops to use workspace dependency

  # ============================================================================
  # Version conflict scenarios
  # ============================================================================

  Scenario: Consolidation skips dependencies with conflicting versions
    Given a repo with conflicting duplicate dependency versions
    And a depguard receipt for conflicting duplicate versions
    When I run buildfix plan
    Then the plan contains no duplicate dependency consolidation fix

  Scenario: Consolidation selects single consistent version
    Given a repo with duplicate dependency versions across members
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    And the workspace dependency uses the selected version from receipt

  # ============================================================================
  # Feature preservation scenarios
  # ============================================================================

  Scenario: Consolidation preserves dependency features
    Given a repo with duplicate dependency versions and features
    And a depguard receipt for duplicate dependency versions with features
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    When I run buildfix apply with --apply
    Then the crate-a Cargo.toml uses workspace dependency for serde with feature "derive"

  Scenario: Consolidation preserves dev-dependency section
    Given a repo with duplicate dev-dependency versions
    And a depguard receipt for duplicate dev-dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    When I run buildfix apply with --apply
    Then the crate-a Cargo.toml uses workspace dev-dependency for serde

  Scenario: Consolidation preserves optional flag
    Given a repo with duplicate optional dependency versions
    And a depguard receipt for duplicate optional dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    And the preserved args include optional flag

  # ============================================================================
  # Multiple dependencies scenarios
  # ============================================================================

  Scenario: Consolidation handles multiple duplicate dependencies
    Given a repo with multiple duplicate dependencies
    And a depguard receipt for multiple duplicate dependencies
    When I run buildfix plan
    Then the plan contains multiple duplicate dependency consolidation fixes

  Scenario: Consolidation creates separate workspace entry per dependency
    Given a repo with multiple duplicate dependencies
    And a depguard receipt for multiple duplicate dependencies
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the root Cargo.toml has multiple workspace dependencies

  # ============================================================================
  # Determinism scenarios
  # ============================================================================

  Scenario: Consolidation fixes are sorted deterministically
    Given a repo with multiple duplicate dependencies
    And a depguard receipt for multiple duplicate dependencies
    When I run buildfix plan
    Then the duplicate deps fixes are sorted by dependency name

  Scenario: Member ops are sorted by manifest path
    Given a repo with duplicate dependency versions across members
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the member ops are sorted by manifest path

  # ============================================================================
  # Receipt integration scenarios
  # ============================================================================

  Scenario: Consolidation responds to depguard sensor
    Given a repo with duplicate dependency versions across members
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix

  Scenario: Consolidation responds to deps.duplicate_dependency_versions check id
    Given a repo with duplicate dependency versions across members
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix

  # ============================================================================
  # Edge cases
  # ============================================================================

  Scenario: Consolidation skips when workspace dependency already exists
    Given a repo with duplicate deps and existing workspace entry
    And a depguard receipt for duplicate dependency versions
    When I run buildfix plan
    Then the plan contains member ops to use workspace dependency
    And the plan does not contain root op to add workspace dependency

  Scenario: Consolidation handles target-specific dependencies
    Given a repo with duplicate target-specific dependencies
    And a depguard receipt for duplicate target-specific dependencies
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    And the preserved args include target cfg

  Scenario: Consolidation handles build-dependencies
    Given a repo with duplicate build-dependency versions
    And a depguard receipt for duplicate build-dependency versions
    When I run buildfix plan
    Then the plan contains a duplicate dependency consolidation fix
    When I run buildfix apply with --apply
    Then the crate-a Cargo.toml uses workspace build-dependency
