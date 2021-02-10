Feature: Remote media disabling

  Scenario: Remote video Track stops on disable
    Given joined Member `Alice`
    And joined Member `Bob`
    When  Member `Alice` disables remote video
    Then `Bob` remote video Track from `Alice` stops

  Scenario: Remote audio Track stops on disable
    Given joined Member `Alice`
    And joined Member `Bob`
    When  Member `Alice` disables remote audio
    Then `Bob` remote audio Track from `Alice` stops
