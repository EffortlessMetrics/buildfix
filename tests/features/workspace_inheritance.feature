Feature: depguard workspace inheritance fixer

  Scenario: Fix normalizes member dependency to workspace=true preserving flags
    Given a repo fixture "workspace-inheritance-drift"
    And receipts contain a finding "depguard/deps.workspace_inheritance/not_inherited"
    When I run "buildfix plan"
    Then the plan includes a toml_transform op "workspace_inheritance_normalize"
    And the transform preserves dependency flags

  Scenario: Conflicting versions are guarded by default
    Given a repo fixture "workspace-inheritance-conflict"
    And receipts contain a finding "depguard/deps.workspace_inheritance/not_inherited"
    When I run "buildfix plan"
    Then the plan marks the operation as guarded
