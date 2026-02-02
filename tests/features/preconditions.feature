Feature: buildfix preconditions

  Scenario: Apply is blocked if repo changed after planning
    Given a repo fixture "workspace-missing-resolver"
    And receipts contain a finding "builddiag/workspace.resolver_v2/missing_or_wrong"
    When I run "buildfix plan"
    And I modify file "Cargo.toml" after planning
    When I run "buildfix apply" with explicit apply opt-in
    Then apply is blocked due to precondition mismatch
    And no repo files were modified by buildfix

  Scenario: Apply proceeds when preconditions match
    Given a repo fixture "workspace-missing-resolver"
    And receipts contain a finding "builddiag/workspace.resolver_v2/missing_or_wrong"
    When I run "buildfix plan"
    When I run "buildfix apply" with explicit apply opt-in
    Then apply succeeded
    And the repo now contains workspace resolver v2
