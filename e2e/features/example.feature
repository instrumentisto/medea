Feature: An example feature
  Scenario: Connection creates on join
    Given joined Member `Alice`
    And Member `Bob`
    When `Bob` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Connection will be created if tracks disabled
    Given Member `Alice` with disabled all
    And joined Member `Bob`
    When `Alice` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Connection will not be created if no Endpoints between Members
    Given empty Member `Alice`
    And joined empty Member `Bob`
    When `Alice` joins Room
    Then `Alice` doesn't receives Connection with Member `Bob`
    And `Bob` doesn't receives Connection with Member `Alice`
