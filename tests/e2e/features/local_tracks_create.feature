Feature: Local tracks are created

  Scenario: Local tracks are created when connecting
    Given room with member Alice
    And joined member Bob
    When Alice joins the room
    Then Alice has 2 local tracks
    And Alice has local device video
    And Alice has local audio

  Scenario: Local tracks are not created when all media is disabled
    Given room with member Alice with disabled media publishing
    And joined member Bob
    When Alice joins the room
    Then Alice has 0 local tracks

  Scenario: Local video track is created when member enables video
    Given room with joined member Alice with disabled media publishing
    And joined member Bob
    When Alice enables video and waits for success
    Then Alice has 1 local tracks
    And Alice has local device video

  Scenario: Local audio track is created when member enables audio
    Given room with joined member Alice with disabled media publishing
    And joined member Bob
    When Alice enables audio and waits for success
    Then Alice has 1 local tracks
    And Alice has local audio
