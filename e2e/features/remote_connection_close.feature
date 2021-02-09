Feature: Remote Connection closing
  Scenario: Connection.on_close fires on partner Member leave
    Given joined Member `Alice`
    And joined Member `Bob`
    When `Bob`'s Room closed by client
    Then `Alice`'s Connection with `Bob` closes

  Scenario: Connection closes on partner Member delete by Control API
    Given joined Member `Alice`
    And joined Member `Bob`
    When Control API removes Member `Bob`
    Then `Alice`'s Connection with `Bob` closes
