Feature: OnLeave callback of Control API
  Scenario: Member closes Room
    Given joined Member `Alice`
    When `Alice`'s Room closed by client
    Then Control API sends OnLeave callback with `Disconnected` reason for Member `Alice`

  Scenario: Member's Jason object disposed
    Given joined Member `Alice`
    When `Alice`'s Jason object disposes
    Then Control API sends OnLeave callback with `Disconnected` reason for Member `Alice`

  Scenario: Member deleted by Control API
    Given joined Member `Alice`
    When Control API removes Member `Alice`
    Then Control API doesn't sends OnLeave callback for Member `Alice`

  Scenario: Member's Room deleted by Control API
    Given joined Member `Alice`
    When Control API removes Room
    Then Control API doesn't sends OnLeave callback for Member `Alice`
