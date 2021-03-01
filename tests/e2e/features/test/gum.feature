Feature: Get user media requests

  Scenario: foo
    Given room with member Alice
    And Alice's gUM broken
    And joined member Bob
    When Alice joins room
    Then Alice's Room.on_failed_local_stream fires 1 time

  Scenario: foobar
    Given room with joined member Alice and Bob with disabled media publishing
    And Alice's gUM broken
    When Alice tries to enable media publishing
    Then Alice's Room.on_failed_local_stream fires 1 time

#  Scenario: bar
#    Given room with joined member Alice and Bob
#    And Alice's gUM broken
#    When Alice disables media in constraints
#    And Alice enables all
#    And Alice enables video and audio constraints
#    Then Alice's Room.on_failed_local_stream fires 1 time
