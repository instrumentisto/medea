Feature: Remote Connection closing
  Scenario: Connection.on_close fires on partner Member leave
    Given room with joined members Alice and Bob
    When Bob's room closed by client
    Then Alice's connection with Bob closes

  Scenario: Connection closes on partner Member delete by Control API
    Given room with joined members Alice and Bob
    When Control API removes member Bob
    Then Alice's connection with Bob closes
