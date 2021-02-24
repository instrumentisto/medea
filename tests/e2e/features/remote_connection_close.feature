Feature: Remote Connection closing
  # FIXME
#  Scenario: Connection.on_close fires on partner Member leave
#    Given room with joined members Alice and Bob
#    When `Bob`'s Room closed by client
#    Then `Alice`'s Connection with `Bob` closes

  Scenario: Connection closes on partner Member delete by Control API
    Given room with joined members Alice and Bob
    When Control API removes Member `Bob`
    Then `Alice`'s Connection with `Bob` closes
