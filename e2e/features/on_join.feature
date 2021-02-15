Feature: OnJoin callback of Control API

  Scenario: OnJoin fires when Member joins
    Given Member `Alice`
    When `Alice` joins Room
    Then Control API sends OnJoin callback for Member `Alice`
