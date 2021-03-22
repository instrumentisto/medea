Feature: Get user media requests

  Scenario: Member joins Room and theirs `getUserMedia()` errors
    Given room with member Alice
    And Alice's `getUserMedia()` will error
    And joined member Bob
    When Alice joins the room
    Then Alice's `Room.on_failed_local_stream()` fires 1 time

  Scenario: Member tries to enable media publishing and theirs `getUserMedia()` errors
    Given room with joined member Alice and Bob with disabled media publishing
    And Alice's `getUserMedia()` will error
    When Alice enables video with error
    Then Alice's `Room.on_failed_local_stream()` fires 1 time

  Scenario: Member tries to enable audio and video and theirs `getUserMedia()` errors
    Given room with joined member Alice and Bob
    And Alice's `getUserMedia()` will error
    When Alice enables video and audio in local media settings
    Then Alice doesn't have live tracks
