Feature: Get user media requests

  Scenario: gUM request rollbacks
    Given room with joined member Alice and Bob
    Given Alice's gUM broken
    When Alice enables video and audio constraints
    Then Alice's Room.on_failed_local_stream fires 1 time
