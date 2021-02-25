Feature: OnLeave callback of Control API
  Scenario: Member closes Room
    Given room with joined member Alice
    When Alice's room closed by client
    Then Control API sends OnLeave callback with `Disconnected` reason for member Alice

  Scenario: Member's Jason object disposed
    Given room with joined member Alice
    When Alice's Jason object disposes
    Then Control API sends OnLeave callback with `Disconnected` reason for member Alice

  Scenario: Member deleted by Control API
    Given room with joined member Alice
    When Control API removes member Alice
    Then Control API doesn't sends OnLeave callback for member `Alice`

  Scenario: Member's Room deleted by Control API
    Given room with joined member Alice
    When Control API removes room
    Then Control API doesn't sends OnLeave callback for member `Alice`
