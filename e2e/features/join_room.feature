Feature: Room joining
  Scenario: Member joined
    Given joined Member `Alice`
    And Member `Bob`
    When `Bob` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Member joined with disabled media
    Given Member `Alice` with disabled all
    And joined Member `Bob`
    When `Alice` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Member without Endpoints joined
    Given empty Member `Alice`
    And joined empty Member `Bob`
    When `Alice` joins Room
    Then `Alice` doesn't receives Connection with Member `Bob`
    And `Bob` doesn't receives Connection with Member `Alice`
