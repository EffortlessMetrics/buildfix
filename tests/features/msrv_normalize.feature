Feature: builddiag MSRV normalization fixer

  Scenario: MSRV normalization is safe when a workspace standard exists
    Given a repo fixture "workspace-msrv-standard-and-member-drift"
    And receipts contain a finding "builddiag/rust.msrv_consistent/mismatch"
    When I run "buildfix plan"
    Then the plan includes toml_set operations setting member rust-version to the workspace value

  Scenario: MSRV normalization is unsafe when no standard exists
    Given a repo fixture "member-msrv-drift-no-workspace-standard"
    And receipts contain a finding "builddiag/rust.msrv_consistent/mismatch"
    When I run "buildfix plan"
    Then the plan marks the operation as unsafe and blocked
