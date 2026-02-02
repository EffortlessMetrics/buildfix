Feature: depguard path dependency requires version fixer

  Scenario: Fix adds version by reading the target crate version
    Given a repo fixture "path-dep-missing-version"
    And receipts contain a finding "depguard/deps.path_requires_version/missing_version"
    When I run "buildfix plan"
    Then the plan includes a toml_set op adding a version from the target crate

  Scenario: Fix is unsafe when target crate version cannot be determined
    Given a repo fixture "path-dep-missing-version-and-target-version-missing"
    And receipts contain a finding "depguard/deps.path_requires_version/missing_version"
    When I run "buildfix plan"
    Then the plan marks the operation as unsafe and blocked
