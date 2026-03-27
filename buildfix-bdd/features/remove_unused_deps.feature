Feature: Remove Unused Dependencies

  # ============================================================================
  # Safety classification scenarios
  # ============================================================================

  Scenario: Unused dependency removal is classified as unsafe
    Given a repo with an unused dependency
    And a cargo-machete receipt for unused dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix
    And the unused dep removal fix has safety class "unsafe"

  Scenario: Unused dependency removal requires --allow-unsafe
    Given a repo with an unused dependency
    And a cargo-machete receipt for unused dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix
    When I run buildfix apply with --apply expecting policy block
    Then the crate-a Cargo.toml still has dependency "serde"

  Scenario: Unused dependency removal applies with --allow-unsafe
    Given a repo with an unused dependency
    And a cargo-machete receipt for unused dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix
    When I run buildfix apply with --apply --allow-unsafe
    Then the crate-a Cargo.toml no longer contains dependency "serde"

  # ============================================================================
  # cargo-machete integration scenarios
  # ============================================================================

  Scenario: Remove unused deps responds to cargo-machete sensor
    Given a repo with an unused dependency
    And a cargo-machete receipt for unused dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix

  Scenario: Remove unused deps responds to machete.unused_dependency check id
    Given a repo with an unused dependency
    And a cargo-machete receipt for unused dependency with check id "machete.unused_dependency"
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix

  # ============================================================================
  # cargo-udeps integration scenarios
  # ============================================================================

  Scenario: Remove unused deps responds to cargo-udeps sensor
    Given a repo with an unused dependency
    And a cargo-udeps receipt for unused dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix

  Scenario: Remove unused deps responds to udeps.unused_dependency check id
    Given a repo with an unused dependency
    And a cargo-udeps receipt for unused dependency with check id "udeps.unused_dependency"
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix

  Scenario: Remove unused deps responds to deps.unused_dependency check id
    Given a repo with an unused dependency
    And a cargo-udeps receipt for unused dependency with check id "deps.unused_dependency"
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix

  # ============================================================================
  # Dependency section scenarios
  # ============================================================================

  Scenario: Remove unused deps handles dev-dependencies
    Given a repo with an unused dev-dependency
    And a cargo-machete receipt for unused dev-dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix
    When I run buildfix apply with --apply --allow-unsafe
    Then the crate-a Cargo.toml no longer contains dev-dependency "tempfile"

  Scenario: Remove unused deps handles build-dependencies
    Given a repo with an unused build-dependency
    And a cargo-machete receipt for unused build-dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix
    When I run buildfix apply with --apply --allow-unsafe
    Then the crate-a Cargo.toml no longer contains build-dependency "cc"

  Scenario: Remove unused deps handles target-specific dependencies
    Given a repo with an unused target-specific dependency
    And a cargo-machete receipt for unused target-specific dependency
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix
    When I run buildfix apply with --apply --allow-unsafe
    Then the crate-a Cargo.toml no longer contains target-specific dependency

  # ============================================================================
  # Multiple unused dependencies scenarios
  # ============================================================================

  Scenario: Remove unused deps handles multiple unused dependencies
    Given a repo with multiple unused dependencies
    And a cargo-machete receipt for multiple unused dependencies
    When I run buildfix plan
    Then the plan contains multiple unused dependency removal fixes

  Scenario: Remove unused deps removes all with single --allow-unsafe
    Given a repo with multiple unused dependencies
    And a cargo-machete receipt for multiple unused dependencies
    When I run buildfix plan
    Then the plan contains multiple unused dependency removal fixes
    When I run buildfix apply with --apply --allow-unsafe
    Then the crate-a Cargo.toml no longer contains any unused dependencies

  # ============================================================================
  # Determinism scenarios
  # ============================================================================

  Scenario: Unused dep removal fixes are sorted deterministically
    Given a repo with multiple unused dependencies
    And a cargo-machete receipt for multiple unused dependencies
    When I run buildfix plan
    Then the unused dep removal fixes are sorted by manifest path and toml path

  # ============================================================================
  # Edge cases
  # ============================================================================

  Scenario: Remove unused deps skips non-existent dependency entries
    Given a repo with an unused dependency
    And a cargo-machete receipt for already-removed dependency
    When I run buildfix plan
    Then the plan contains no unused dependency removal fix

  Scenario: Remove unused deps handles invalid toml_path gracefully
    Given a repo with an unused dependency
    And a cargo-machete receipt with invalid toml_path
    When I run buildfix plan
    Then the plan contains no unused dependency removal fix

  Scenario: Remove unused deps handles missing toml_path with fallback
    Given a repo with an unused dependency
    And a cargo-machete receipt with dep and table but no toml_path
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix

  # ============================================================================
  # Multiple sensors scenarios
  # ============================================================================

  Scenario: Remove unused deps deduplicates findings from multiple sensors
    Given a repo with an unused dependency
    And a cargo-machete receipt for unused dependency
    And a cargo-udeps receipt for the same unused dependency
    When I run buildfix plan
    Then the plan contains exactly 1 unused dependency removal fix

  # ============================================================================
  # Receipt data extraction scenarios
  # ============================================================================

  Scenario: Remove unused deps extracts dep from dep field
    Given a repo with an unused dependency
    And a cargo-machete receipt with dep field "serde"
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix for "serde"

  Scenario: Remove unused deps extracts dep from dependency field
    Given a repo with an unused dependency
    And a cargo-machete receipt with dependency field "serde"
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix for "serde"

  Scenario: Remove unused deps extracts dep from crate field
    Given a repo with an unused dependency
    And a cargo-udeps receipt with crate field "serde"
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix for "serde"

  Scenario: Remove unused deps extracts dep from name field
    Given a repo with an unused dependency
    And a cargo-udeps receipt with name field "serde"
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix for "serde"

  # ============================================================================
  # Workspace member scenarios
  # ============================================================================

  Scenario: Remove unused deps handles dependencies in workspace members
    Given a workspace with an unused dependency "log"
    And a cargo-machete receipt for unused dependency in member crate
    When I run buildfix plan
    Then the plan contains an unused dependency removal fix
    And the fix targets the member crate Cargo.toml

  Scenario: Remove unused deps handles dependencies in multiple workspace members
    Given a workspace with unused dependencies in multiple members
    And cargo-machete receipts for unused dependencies in multiple members
    When I run buildfix plan
    Then the plan contains unused dependency removal fixes for each member
