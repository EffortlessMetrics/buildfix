Feature: Path Dependency Version Normalization

  Background:
    Given a repo with path dependencies missing versions

  # ============================================================================
  # Safety classification scenarios
  # ============================================================================

  Scenario: Path dependency version is classified as safe when version can be inferred
    Given a depguard receipt for path dependency missing version
    And the target crate has version "1.0.0"
    When I run buildfix plan
    Then the plan contains a path dep version fix
    And the path dep version fix has safety class "safe"

  Scenario: Path dependency version applies automatically with --apply
    Given a depguard receipt for path dependency missing version
    And the target crate has version "1.0.0"
    When I run buildfix plan
    Then the plan contains a path dep version fix
    When I run buildfix apply with --apply
    Then the dependency has version "1.0.0"

  Scenario: Path dependency version is unsafe when target crate has no version
    Given a depguard receipt for path dependency missing version
    And the target crate has no version field
    And the workspace has no package.version
    When I run buildfix plan
    Then the plan contains a path dep version fix
    And the path dep version fix has safety class "unsafe"
    And the path dep version fix requires parameter "version"

  # ============================================================================
  # Version inference scenarios
  # ============================================================================

  Scenario: Path dependency version is inferred from target crate Cargo.toml
    Given a depguard receipt for path dependency missing version
    And a crate at path "crates/utils" with version "2.1.0"
    And a dependency on "utils" with path "../utils"
    When I run buildfix plan
    Then the plan contains a path dep version fix
    And the path dep version fix targets version "2.1.0"

  Scenario: Path dependency version falls back to workspace package version
    Given a depguard receipt for path dependency missing version
    And the target crate has no version field
    And a workspace package version "3.0.0"
    When I run buildfix plan
    Then the plan contains a path dep version fix
    And the path dep version fix targets version "3.0.0"

  Scenario: Path dependency version prefers target crate version over workspace
    Given a depguard receipt for path dependency missing version
    And a crate at path "crates/utils" with version "2.1.0"
    And a workspace package version "3.0.0"
    When I run buildfix plan
    Then the plan contains a path dep version fix
    And the path dep version fix targets version "2.1.0"

  Scenario Outline: Path dependency version handles various version formats
    Given a depguard receipt for path dependency missing version
    And the target crate has version "<version>"
    When I run buildfix plan
    Then the plan contains a path dep version fix
    And the path dep version fix targets version "<version>"

    Examples:
      | version |
      | 1.0.0   |
      | 0.1.0   |
      | 2.1.0   |
      | 0.0.1   |
      | 1.2.3   |
      | 10.0.0  |

  # ============================================================================
  # Dependency section scenarios
  # ============================================================================

  Scenario: Path dependency version adds version to dependencies section
    Given a depguard receipt for path dependency missing version in "dependencies"
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the dependency in "dependencies" has version "1.0.0"

  Scenario: Path dependency version adds version to dev-dependencies section
    Given a depguard receipt for path dependency missing version in "dev-dependencies"
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the dependency in "dev-dependencies" has version "1.0.0"

  Scenario: Path dependency version adds version to build-dependencies section
    Given a depguard receipt for path dependency missing version in "build-dependencies"
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the dependency in "build-dependencies" has version "1.0.0"

  Scenario: Path dependency version handles target-specific dependencies
    Given a depguard receipt for path dependency missing version in "target.cfg(unix).dependencies"
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the dependency in "target.cfg(unix).dependencies" has version "1.0.0"

  # ============================================================================
  # Multiple path dependencies scenarios
  # ============================================================================

  Scenario: Path dependency version handles multiple path dependencies
    Given a workspace with multiple path dependencies missing versions
    And all target crates have versions
    And a depguard receipt for multiple missing versions
    When I run buildfix plan
    Then the plan contains multiple path dep version fixes
    And all path dep version fixes have safety class "safe"

  Scenario: Path dependency version handles mixed safety classifications
    Given a workspace with path dependencies
    And crate-a target has version "1.0.0"
    And crate-b target has no version
    And a depguard receipt for multiple missing versions
    When I run buildfix plan
    Then the plan contains path dep version fixes with mixed safety classes

  Scenario: Path dependency version skips dependencies that already have version
    Given a depguard receipt for path dependency missing version
    And a dependency on "utils" with path "../utils" and version "1.0.0"
    When I run buildfix plan
    Then the plan contains no path dep version fix for "utils"

  # ============================================================================
  # Workspace dependency scenarios
  # ============================================================================

  Scenario: Path dependency version skips workspace = true dependencies
    Given a depguard receipt for path dependency missing version
    And a dependency using workspace inheritance with path
    When I run buildfix plan
    Then the plan contains no path dep version fix for inherited dependency

  Scenario: Path dependency version does not modify workspace = true entries
    Given a workspace with path dependencies
    And crate-a using workspace = true for dependency
    And crate-b with explicit path dependency missing version
    And a depguard receipt for crate-b only
    When I run buildfix plan
    Then the plan contains exactly 1 path dep version fix
    And the path dep version fix targets crate-b

  # ============================================================================
  # Receipt source scenarios
  # ============================================================================

  Scenario: Path dependency version consumes depguard receipts
    Given a depguard receipt for path dependency missing version with check "deps.path_requires_version"
    When I run buildfix plan
    Then the plan contains a path dep version fix

  Scenario Outline: Path dependency version handles various check IDs
    Given a depguard receipt for path dependency missing version with check "<check_id>"
    When I run buildfix plan
    Then the plan contains a path dep version fix

    Examples:
      | check_id                         |
      | deps.path_requires_version       |
      | cargo.path_requires_version      |
      | dependency.path_missing_version  |

  # ============================================================================
  # Idempotency scenarios
  # ============================================================================

  Scenario: Path dependency version fix is idempotent
    Given a depguard receipt for path dependency missing version
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    And I run buildfix plan again
    Then the plan contains no path dep version fixes

  Scenario: Path dependency version does not re-add version if already present
    Given a depguard receipt for path dependency missing version
    And a dependency on "utils" with path "../utils" and version "1.0.0"
    When I run buildfix plan
    Then the plan contains no path dep version fix

  # ============================================================================
  # Determinism scenarios
  # ============================================================================

  Scenario: Path dependency version fixes are sorted deterministically
    Given a workspace with multiple path dependencies missing versions
    And all target crates have versions
    And a depguard receipt for multiple missing versions
    When I run buildfix plan
    Then the path dep version fixes are sorted by manifest path

  Scenario: Path dependency version produces stable output across runs
    Given a depguard receipt for path dependency missing version
    And the target crate has version "1.0.0"
    When I run buildfix plan twice
    Then both plans are identical

  # ============================================================================
  # Edge cases
  # ============================================================================

  Scenario: Path dependency version handles inline table dependency format
    Given a depguard receipt for path dependency missing version
    And an inline table dependency 'utils = { path = "../utils" }'
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the dependency has version "1.0.0"

  Scenario: Path dependency version handles standard table dependency format
    Given a depguard receipt for path dependency missing version
    And a standard table dependency '[dependencies.utils] path = "../utils"'
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the dependency has version "1.0.0"

  Scenario: Path dependency version handles relative paths correctly
    Given a depguard receipt for path dependency missing version
    And a crate at "crates/deeply/nested/utils" with dependency path "../../../shared"
    And the shared crate has version "1.0.0"
    When I run buildfix plan
    Then the plan contains a path dep version fix
    And the path dep version fix targets version "1.0.0"

  Scenario: Path dependency version blocked when target crate Cargo.toml is unreadable
    Given a depguard receipt for path dependency missing version
    And the target crate Cargo.toml does not exist
    And the workspace has no package.version
    When I run buildfix plan
    Then the plan contains a path dep version fix
    And the path dep version fix has safety class "unsafe"

  # ============================================================================
  # TOML preservation scenarios
  # ============================================================================

  Scenario: Path dependency version preserves existing dependency fields
    Given a depguard receipt for path dependency missing version
    And a dependency 'utils = { path = "../utils", features = ["async"] }'
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the dependency has version "1.0.0"
    And the dependency preserves features ["async"]

  Scenario: Path dependency version preserves comments in Cargo.toml
    Given a depguard receipt for path dependency missing version
    And a Cargo.toml with comments
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the Cargo.toml preserves comments

  Scenario: Path dependency version preserves formatting where possible
    Given a depguard receipt for path dependency missing version
    And a Cargo.toml with specific formatting
    And the target crate has version "1.0.0"
    When I run buildfix plan
    And I run buildfix apply with --apply
    Then the Cargo.toml formatting is preserved
