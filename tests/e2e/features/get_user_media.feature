Feature: `getUserMedia()` requests

  Scenario: Member joins Room and its `getUserMedia()` errors
    Given room with member Alice
    And Alice's `getUserMedia()` errors
    And joined member Bob
    When Alice joins the room
    Then Alice's `Room.on_failed_local_stream()` fires 1 time

  Scenario: Member tries to enable media publishing and its `getUserMedia()` errors
    Given room with joined member Alice and Bob with disabled media publishing
    And Alice's `getUserMedia()` errors
    When Alice enables video with error
    Then Alice's `Room.on_failed_local_stream()` fires 1 time

  Scenario: Member tries to enable audio and video and its `getUserMedia()` errors
    Given room with joined member Alice and Bob
    And Alice's `getUserMedia()` errors
    When Alice enables video and audio in local media settings
    Then Alice doesn't have live local tracks
