Feature: buildfix plan and apply

  Scenario: Plan is empty when there are no fixable findings
    Given a repo fixture "empty-workspace"
    And receipts contain only non-fixable findings
    When I run "buildfix plan"
    Then the plan has 0 operations
    And the patch diff is empty

  Scenario: Denied fix is planned but blocked
    Given a repo fixture "workspace-missing-resolver"
    And receipts contain a finding "builddiag/workspace.resolver_v2/missing_or_wrong"
    And buildfix policy denies "builddiag/workspace.resolver_v2/*"
    When I run "buildfix plan"
    Then the plan contains a blocked operation with reason "policy.denied"

  Scenario: Apply requires explicit opt-in
    Given a repo fixture "workspace-missing-resolver"
    And receipts contain a finding "builddiag/workspace.resolver_v2/missing_or_wrong"
    When I run "buildfix plan"
    And I run "buildfix apply" without explicit apply opt-in
    Then apply is blocked with reason "policy.apply_not_enabled"
