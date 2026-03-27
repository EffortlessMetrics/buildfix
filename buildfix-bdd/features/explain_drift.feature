Feature: Explain metadata drift detection
  As a buildfix developer
  I want to ensure that explain output matches fixer implementation
  So that documentation stays in sync with code

  Scenario: All fix explanations match catalog entries
    Given the fixer catalog is enabled
    When I query all fix explanations
    Then each explanation should match its corresponding catalog entry
    And each catalog entry should have a matching explanation

  Scenario: Fix explanation safety classifications are consistent
    Given the fixer catalog is enabled
    When I query fix explanations by safety class
    Then safe fixes should have safety "safe"
    And guarded fixes should have safety "guarded"
    And unsafe fixes should have safety "unsafe"

  Scenario: Fix explanation triggers match catalog triggers
    Given the fixer catalog is enabled
    When I query fix explanation triggers
    Then each explanation's triggers should match its catalog entry's triggers

  Scenario: Fix explanation keys are unique
    Given the fixer catalog is enabled
    When I query all fix explanations
    Then each explanation should have a unique key
    And each explanation should have a unique fix_id

  Scenario: Fix lookup by key works for all catalog entries
    Given the fixer catalog is enabled
    When I look up fixes by key
    Then each catalog entry key should resolve to the correct explanation

  Scenario: Fix lookup by fix_id works for all catalog entries
    Given the fixer catalog is enabled
    When I look up fixes by fix_id
    Then each catalog entry fix_id should resolve to the correct explanation

  Scenario: All fix explanations have substantive documentation
    Given the fixer catalog is enabled
    When I query all fix explanations
    Then each explanation should have a description of at least 50 characters
    And each explanation should have a safety rationale of at least 50 characters
    And each explanation should have remediation guidance

  Scenario: Fix explanations follow consistent naming conventions
    Given the fixer catalog is enabled
    When I query all fix explanations
    Then each title should use title case
    And each key should use hyphens not underscores

  Scenario: Policy keys are generated correctly for all triggers
    Given the fixer catalog is enabled
    When I generate policy keys for each fix
    Then each policy key should follow the format "sensor/check_id/code"
    And policy keys should be sorted and deduplicated
