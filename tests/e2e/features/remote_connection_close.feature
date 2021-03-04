Feature: Remote Connection closing
  Scenario: Connection closes when partner Member is deleted by Control API
    Given room with joined members Alice and Bob
    When Control API removes member Bob
    Then Alice's connection with Bob closes

  Scenario: Connection closes when partner Member disposes Jason
    Given room with joined members Alice and Bob
    When Bob disposes Jason object
    Then Alice's connection with Bob closes
