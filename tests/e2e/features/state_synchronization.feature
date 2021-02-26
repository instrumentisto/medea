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

  Scenario: Disable/enable works fine while disconnect
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Alice disables audio
    And Alice enables audio
    And Alice restores WS connection
    Then Bob's audio remote track with Alice is enabled

  Scenario: Audio endpoint added while disconnect
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Alice loses WS connection
    And Control API interconnected audio of Alice and Bob
    And Alice restores WS connection
    Then Alice has audio remote tracks with Bob
    And Bob has audio remote tracks with Alice

  Scenario: Video endpoint added while disconnect
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Alice loses WS connection
    And Control API interconnected video of Alice and Bob
    And Alice restores WS connection
    Then Alice has video remote tracks with Bob
    And Bob has video remote tracks with Alice

  Scenario: New Endpoint creates new Tracks
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Alice loses WS connection
    And Control API interconnects Alice and Bob
    And Alice restores WS connection
    Then Alice has audio and video remote tracks with Bob
    And Bob has audio and video remote tracks with Alice

  Scenario: New Member joins while disconnect
    Given room with joined member Alice
    And member Bob
    When Alice loses WS connection
    And Bob joins room
    And Alice restores WS connection
    Then Alice receives connection with Bob
    And Bob receives connection with Alice

  Scenario: Connection.on_close fires on partner Member leave
    Given room with joined members Alice and Bob
    When Alice loses WS connection
    And Bob's room closed by client
    And Alice restores WS connection
    Then Alice's connection with Bob closes

  Scenario: Connection closes on partner Member delete by Control API
    Given room with joined members Alice and Bob
    When Alice loses WS connection
    And Control API removes member Bob
    And Alice restores WS connection
    Then Alice's connection with Bob closes
