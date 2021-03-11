Feature: `OnLeave` callback of Control API

  Scenario: Member closes room
    Given room with joined member Alice
    When Alice's room closed by client
    Then Control API sends `OnLeave` callback with `Disconnected` reason for member Alice

  Scenario: Member's Jason object disposed
    Given room with joined member Alice
    When Alice disposes Jason object
    Then Control API sends `OnLeave` callback with `Disconnected` reason for member Alice

  Scenario: Member deleted by Control API
    Given room with joined member Alice
    When Control API removes member Alice
    Then Control API doesn't send `OnLeave` callback for member Alice

  Scenario: Member's room deleted by Control API
    Given room with joined member Alice
    When Control API removes the room
    Then Control API doesn't send `OnLeave` callback for member Alice
