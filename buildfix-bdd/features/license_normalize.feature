Feature: License Normalization

  Background:
    Given a repo with inconsistent license

  # ============================================================================
  # Safety classification scenarios
  # ============================================================================

  Scenario: License normalization is classified as guarded by default
    Given a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    Then the plan contains a license normalization fix
    And the license fix has safety class "guarded"

  Scenario: License normalization requires guarded opt-in
    Given a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    Then the plan contains a license normalization fix
    When I run buildfix apply with --apply expecting policy block
    Then the crate-a Cargo.toml has license "MIT"

  Scenario: License normalization applies with --allow-guarded
    Given a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    Then the plan contains a license normalization fix
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has license "MIT OR Apache-2.0"

  Scenario: License normalization is safe with high confidence and full consensus
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And all workspace crates agree on license
    And a cargo-deny receipt for license inconsistency with confidence 0.95
    When I run buildfix plan
    Then the plan contains a license normalization fix
    And the license fix has safety class "safe"

  Scenario: License normalization is unsafe without workspace canonical
    Given a repo with no canonical license
    And a crate with license "MIT"
    And a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    Then the plan contains a license normalization fix
    And the license fix has safety class "unsafe"

  # ============================================================================
  # Workspace standard scenarios
  # ============================================================================

  Scenario: License normalization uses workspace.package.license as canonical
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And a crate with license "MIT"
    And a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    Then the plan contains a license normalization fix
    And the license fix targets license "MIT OR Apache-2.0"

  Scenario: License normalization falls back to root package license
    Given a repo with root package license "Apache-2.0" but no workspace package license
    And a crate with license "MIT"
    And a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    Then the plan contains a license normalization fix
    And the license fix targets license "Apache-2.0"

  # ============================================================================
  # Missing license scenarios
  # ============================================================================

  Scenario: License normalization adds missing license field
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And a crate with missing license field
    And a cargo-deny receipt for missing license
    When I run buildfix plan
    Then the plan contains a license normalization fix
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has license "MIT OR Apache-2.0"

  Scenario: License normalization blocked when no canonical license exists
    Given a repo with no canonical license
    And a crate with missing license field
    And a cargo-deny receipt for missing license
    When I run buildfix plan
    Then the plan contains a license normalization fix
    And the license fix has safety class "unsafe"
    And the license fix requires parameter "license"

  # ============================================================================
  # Multiple license formats scenarios
  # ============================================================================

  Scenario Outline: License normalization handles various license formats
    Given a repo with workspace package license "<canonical>"
    And a crate with license "<current>"
    And a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    Then the plan contains a license normalization fix
    And the license fix targets license "<canonical>"

    Examples:
      | canonical          | current    |
      | MIT                | Apache-2.0 |
      | Apache-2.0         | MIT        |
      | MIT OR Apache-2.0  | MIT        |
      | MIT OR Apache-2.0  | Apache-2.0 |
      | MIT AND Apache-2.0 | MIT        |
      | ISC                | MIT        |
      | BSD-3-Clause       | MIT        |

  Scenario: License normalization preserves SPDX expression format
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And a crate with license "MIT"
    And a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    And I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has license "MIT OR Apache-2.0"

  # ============================================================================
  # Multiple crates scenarios
  # ============================================================================

  Scenario: License normalization handles multiple crates with different licenses
    Given a workspace with multiple crates having different licenses
    And a cargo-deny receipt for multiple license inconsistencies
    When I run buildfix plan
    Then the plan contains multiple license normalization fixes
    And all license fixes target the same canonical license

  Scenario: License normalization skips crates already at canonical license
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And crate-a with license "MIT OR Apache-2.0"
    And crate-b with license "MIT"
    And a cargo-deny receipt for license inconsistency for crate-b only
    When I run buildfix plan
    Then the plan contains exactly 1 license normalization fix
    And the license fix targets crate-b

  Scenario: License normalization handles workspace with all crates missing license
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And all crates with missing license field
    And a cargo-deny receipt for all missing licenses
    When I run buildfix plan
    Then the plan contains license normalization fixes for all crates

  # ============================================================================
  # Workspace inheritance scenarios
  # ============================================================================

  Scenario: License normalization supports workspace inheritance
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And a crate using license workspace inheritance
    And a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    Then the plan contains no license normalization fix for inherited license

  Scenario: License normalization does not modify workspace = true entries
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And crate-a with license workspace = true
    And crate-b with license "MIT"
    And a cargo-deny receipt for license inconsistency for crate-b only
    When I run buildfix plan
    Then the plan contains exactly 1 license normalization fix
    And the license fix targets crate-b

  # ============================================================================
  # Receipt source scenarios
  # ============================================================================

  Scenario: License normalization consumes cargo-deny receipts
    Given a cargo-deny receipt for license inconsistency with check "licenses.missing"
    When I run buildfix plan
    Then the plan contains a license normalization fix

  Scenario: License normalization consumes deny receipts
    Given a deny receipt for license inconsistency with check "license.unlicensed"
    When I run buildfix plan
    Then the plan contains a license normalization fix

  Scenario Outline: License normalization handles various check IDs
    Given a cargo-deny receipt for license inconsistency with check "<check_id>"
    When I run buildfix plan
    Then the plan contains a license normalization fix

    Examples:
      | check_id                   |
      | licenses.unlicensed        |
      | licenses.missing           |
      | licenses.missing_license   |
      | license.unlicensed         |
      | license.missing            |
      | license.missing_license    |
      | cargo.licenses.unlicensed  |
      | cargo.licenses.missing_license |

  # ============================================================================
  # Determinism scenarios
  # ============================================================================

  Scenario: License normalization fixes are sorted deterministically
    Given a workspace with multiple crates having different licenses
    And a cargo-deny receipt for multiple license inconsistencies
    When I run buildfix plan
    Then the license fixes are sorted by manifest path

  # ============================================================================
  # Edge cases
  # ============================================================================

  Scenario: License normalization handles empty license string
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And a crate with empty license string
    And a cargo-deny receipt for missing license
    When I run buildfix plan
    Then the plan contains a license normalization fix
    And the license fix targets license "MIT OR Apache-2.0"

  Scenario: License normalization handles crate without package section
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And a crate without package section
    And a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    Then the plan contains no license normalization fix for invalid manifest

  Scenario: License normalization is idempotent
    Given a repo with workspace package license "MIT OR Apache-2.0"
    And a crate with license "MIT"
    And a cargo-deny receipt for license inconsistency
    When I run buildfix plan
    And I run buildfix apply with --apply --allow-guarded
    And I run buildfix plan again
    Then the plan contains no license normalization fix
