Feature: Get user media requests

  Scenario: Member joins Room with broken gUM
    Given room with member Alice
    And Alice's gUM broken
    And joined member Bob
    When Alice joins the room
    Then Alice's Room.on_failed_local_stream fires 1 time

  Scenario: Member tries to enable media publishing while gUM is broken
    Given room with joined member Alice and Bob with disabled media publishing
    And Alice's gUM broken
    When Alice tries to enable media publishing
    Then Alice's Room.on_failed_local_stream fires 1 time

  Scenario: Member tries to enable audio and video constraints while gUM is broken
    Given room with joined member Alice and Bob
    And Alice's gUM broken
    When Alice enables video and audio constraints
    Then Alice doesn't have live tracks
