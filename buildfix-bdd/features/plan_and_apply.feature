Feature: Plan and apply

  Scenario: Adds workspace resolver v2
    Given a repo missing workspace resolver v2
    And a builddiag receipt for resolver v2
    When I run buildfix plan
    Then the plan contains a resolver v2 fix
    When I run buildfix apply with --apply
    Then the root Cargo.toml sets workspace resolver to "2"
