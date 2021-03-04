Feature: Send Media disabling

  Scenario: Member disables video during call
    Given room with joined member Alice and Bob
    When Bob disables video
    Then Alice's device video remote track with Bob is disabled
    And Alice's audio remote track with Bob is enabled

  Scenario: Member disables audio during call
    Given room with joined member Alice and Bob
    When Bob disables audio
    Then Alice's audio remote track with Bob is disabled
    And Alice's device video remote track with Bob is enabled

  Scenario: Member disables video before call
    Given room with joined member Alice
    And member Bob with disabled video publishing
    When Bob joins room
    Then Alice doesn't have device video remote track with Bob
    And Alice's audio remote track with Bob is enabled

  Scenario: Member disables audio before call
    Given room with joined member Alice
    And member Bob with disabled audio publishing
    When Bob joins room
    Then Alice doesn't have audio remote track with Bob
    And Alice's device video remote track with Bob is enabled

  Scenario: Member enables audio during call
    Given room with joined member Alice
    And member Bob with disabled audio publishing
    When Bob joins room
    And Bob enables audio
    Then Alice's audio remote track with Bob is enabled

  Scenario: Member enables video during call
    Given room with joined member Alice
    And member Bob with disabled video publishing
    When Bob joins room
    And Bob enables video
    Then Alice's device video remote track with Bob is enabled

  Scenario: Local Track is dropped on video disable
    Given room with joined member Alice and Bob
    When Bob disables video
    Then Bob's device video local track is stopped

  Scenario: Local Track is dropped on audio disable
    Given room with joined member Alice and Bob
    When Bob disables audio
    Then Bob's audio local track is stopped
