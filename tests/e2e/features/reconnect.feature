Feature: Reconnects

  Scenario: Remote track disable works while disconnect
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Bob disables audio
    And Alice restores WS connection
    Then Alice's audio remote track with Bob is disabled

  Scenario: Local track disable works while disconnect
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Alice disables audio
    And Alice restores WS connection
    Then Bob's audio remote track with Alice is disabled
    And Alice's audio local track is disabled

  Scenario: Disable/enable works fine while disconnect
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Alice disables audio
    And Alice enables audio
    And Alice restores WS connection
    Then Bob's audio remote track with Alice is enabled
    And Alice's audio local track is enabled
