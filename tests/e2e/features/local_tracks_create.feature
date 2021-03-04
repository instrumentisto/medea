Feature: Local Track are created

  Scenario: Local Tracks are created on connect
    Given room with member Alice
    And joined member Bob
    When Alice joins room
    Then Member Alice has 2 local tracks
    And Alice has local device video
    And Alice has local audio

  Scenario: Local Tracks are not created if all media is disabled
    Given room with member Alice with disabled media publishing
    And joined member Bob
    When Alice joins room
    Then Member Alice has 0 local tracks

  Scenario: Local video Track is created when Member enables video
    Given room with joined member Alice with disabled media publishing
    And joined member Bob
    When Alice enables video
    Then Member Alice has 1 local tracks
    And Alice has local device video

  Scenario: Local audio Track is created when Member enables audio
    Given joined member Alice with disabled media publishing
    And joined member Bob
    When Alice enables audio
    Then Member Alice has 1 local tracks
    And Alice has local audio
