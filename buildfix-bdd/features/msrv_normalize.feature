Feature: MSRV Normalization

  Background:
    Given a repo with inconsistent MSRV

  # ============================================================================
  # Safety classification scenarios
  # ============================================================================

  Scenario: MSRV normalization is classified as guarded by default
    Given a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix has safety class "guarded"

  Scenario: MSRV normalization requires guarded opt-in
    Given a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    When I run buildfix apply with --apply expecting policy block
    Then the crate-a Cargo.toml has rust-version "1.65"

  Scenario: MSRV normalization applies with --allow-guarded
    Given a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has rust-version "1.70"

  Scenario: MSRV normalization is safe with high confidence and full consensus
    Given a repo with workspace package rust-version "1.70"
    And all workspace crates agree on MSRV
    And a builddiag receipt for MSRV inconsistency with confidence 0.95
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix has safety class "safe"

  Scenario: MSRV normalization is unsafe without workspace canonical
    Given a repo with no canonical MSRV
    And a crate with rust-version "1.60"
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix has safety class "unsafe"

  # ============================================================================
  # Workspace standard scenarios
  # ============================================================================

  Scenario: MSRV normalization uses workspace.package.rust-version as canonical
    Given a repo with workspace package rust-version "1.70"
    And a crate with rust-version "1.60"
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix targets rust-version "1.70"

  Scenario: MSRV normalization falls back to root package rust-version
    Given a repo with root package rust-version "1.70" but no workspace package rust-version
    And a crate with rust-version "1.60"
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix targets rust-version "1.70"

  # ============================================================================
  # Missing MSRV scenarios
  # ============================================================================

  Scenario: MSRV normalization adds missing rust-version field
    Given a repo with workspace package rust-version "1.70"
    And a crate with missing rust-version field
    And a builddiag receipt for missing MSRV
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    When I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has rust-version "1.70"

  Scenario: MSRV normalization blocked when no canonical MSRV exists
    Given a repo with no canonical MSRV
    And a crate with missing rust-version field
    And a builddiag receipt for missing MSRV
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix has safety class "unsafe"
    And the MSRV fix requires parameter "rust_version"

  # ============================================================================
  # MSRV version scenarios
  # ============================================================================

  Scenario Outline: MSRV normalization handles various MSRV versions
    Given a repo with workspace package rust-version "<canonical>"
    And a crate with rust-version "<current>"
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix targets rust-version "<canonical>"

    Examples:
      | canonical | current |
      | 1.70      | 1.60    |
      | 1.70      | 1.56    |
      | 1.75      | 1.65    |
      | 1.80      | 1.70    |
      | 1.65      | 1.56    |
      | 1.70.0    | 1.60.0  |

  Scenario: MSRV normalization preserves semver format
    Given a repo with workspace package rust-version "1.70.0"
    And a crate with rust-version "1.60.0"
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    And I run buildfix apply with --apply --allow-guarded
    Then the crate-a Cargo.toml has rust-version "1.70.0"

  # ============================================================================
  # Multiple crates scenarios
  # ============================================================================

  Scenario: MSRV normalization handles multiple crates with different MSRVs
    Given a workspace with multiple crates having different MSRVs
    And a builddiag receipt for multiple MSRV inconsistencies
    When I run buildfix plan
    Then the plan contains multiple MSRV normalization fixes
    And all MSRV fixes target the same canonical rust-version

  Scenario: MSRV normalization skips crates already at canonical MSRV
    Given a repo with workspace package rust-version "1.70"
    And crate-a with rust-version "1.70"
    And crate-b with rust-version "1.60"
    And a builddiag receipt for MSRV inconsistency for crate-b only
    When I run buildfix plan
    Then the plan contains exactly 1 MSRV normalization fix
    And the MSRV fix targets crate-b

  Scenario: MSRV normalization handles workspace with all crates missing MSRV
    Given a repo with workspace package rust-version "1.70"
    And all crates with missing rust-version field
    And a builddiag receipt for all missing MSRVs
    When I run buildfix plan
    Then the plan contains MSRV normalization fixes for all crates

  # ============================================================================
  # Workspace inheritance scenarios
  # ============================================================================

  Scenario: MSRV normalization supports workspace inheritance
    Given a repo with workspace package rust-version "1.70"
    And a crate using rust-version workspace inheritance
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains no MSRV normalization fix for inherited rust-version

  Scenario: MSRV normalization does not modify workspace = true entries
    Given a repo with workspace package rust-version "1.70"
    And crate-a with rust-version workspace = true
    And crate-b with rust-version "1.60"
    And a builddiag receipt for MSRV inconsistency for crate-b only
    When I run buildfix plan
    Then the plan contains exactly 1 MSRV normalization fix
    And the MSRV fix targets crate-b

  # ============================================================================
  # Receipt source scenarios
  # ============================================================================

  Scenario: MSRV normalization consumes builddiag receipts
    Given a builddiag receipt for MSRV inconsistency with check "rust.msrv_consistent"
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix

  Scenario: MSRV normalization consumes cargo receipts
    Given a cargo receipt for MSRV inconsistency with check "cargo.msrv_consistent"
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix

  Scenario Outline: MSRV normalization handles various check IDs
    Given a builddiag receipt for MSRV inconsistency with check "<check_id>"
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix

    Examples:
      | check_id              |
      | rust.msrv_consistent  |
      | cargo.msrv_consistent |
      | msrv.consistent       |

  # ============================================================================
  # Determinism scenarios
  # ============================================================================

  Scenario: MSRV normalization fixes are sorted deterministically
    Given a workspace with multiple crates having different MSRVs
    And a builddiag receipt for multiple MSRV inconsistencies
    When I run buildfix plan
    Then the MSRV fixes are sorted by manifest path

  # ============================================================================
  # Edge cases
  # ============================================================================

  Scenario: MSRV normalization handles empty rust-version string
    Given a repo with workspace package rust-version "1.70"
    And a crate with empty rust-version string
    And a builddiag receipt for missing MSRV
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix targets rust-version "1.70"

  Scenario: MSRV normalization handles crate without package section
    Given a repo with workspace package rust-version "1.70"
    And a crate without package section
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains no MSRV normalization fix for invalid manifest

  Scenario: MSRV normalization is idempotent
    Given a repo with workspace package rust-version "1.70"
    And a crate with rust-version "1.60"
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    And I run buildfix apply with --apply --allow-guarded
    And I run buildfix plan again
    Then the plan contains no MSRV normalization fix

  Scenario: MSRV normalization handles major.minor format
    Given a repo with workspace package rust-version "1.70"
    And a crate with rust-version "1.56"
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix targets rust-version "1.70"

  Scenario: MSRV normalization handles major.minor.patch format
    Given a repo with workspace package rust-version "1.70.0"
    And a crate with rust-version "1.56.1"
    And a builddiag receipt for MSRV inconsistency
    When I run buildfix plan
    Then the plan contains an MSRV normalization fix
    And the MSRV fix targets rust-version "1.70.0"
