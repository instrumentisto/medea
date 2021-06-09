Feature: Media enabling/disabling

  Scenario: Member disables video during call
    Given room with joined members Alice and Bob
    When Bob disables video and waits for success
    Then Alice's device video remote track from Bob is disabled
    And Alice's audio remote track from Bob is enabled

  Scenario: Member disables audio during call
    Given room with joined members Alice and Bob
    When Bob disables audio and waits for success
    Then Alice's audio remote track from Bob is disabled
    And Alice's device video remote track from Bob is enabled

  Scenario: Member disables video before call
    Given room with joined member Alice
    And member Bob with disabled video publishing
    When Bob joins the room
    Then Alice doesn't have device video remote track from Bob
    And Alice's audio remote track from Bob is enabled

  Scenario: Member disables audio before call
    Given room with joined member Alice
    And member Bob with disabled audio publishing
    When Bob joins the room
    Then Alice doesn't have audio remote track from Bob
    And Alice's device video remote track from Bob is enabled

  Scenario: Member enables audio during call
    Given room with joined member Alice
    And member Bob with disabled audio publishing
    When Bob joins the room
    And Bob enables audio and waits for success
    Then Alice's audio remote track from Bob is enabled

  Scenario: Member enables video during call
    Given room with joined member Alice
    And member Bob with disabled video publishing
    When Bob joins the room
    And Bob enables video and waits for success
    Then Alice's device video remote track from Bob is enabled

  Scenario: Local track is dropped when video is disabled
    Given room with joined members Alice and Bob
    When Bob disables video and waits for success
    Then Bob's device video local track is stopped

  Scenario: Local track is dropped when audio is disabled
    Given room with joined members Alice and Bob
    When Bob disables audio and waits for success
    Then Bob's audio local track is stopped
