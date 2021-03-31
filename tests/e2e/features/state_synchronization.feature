Feature: State synchronization

  Scenario: `RoomHandle.on_connection_loss()` fires when WS connection lost
    Given room with joined member Alice with no WebRTC endpoints
    When Alice loses WS connection
    Then Alice's WS connection is lost

  Scenario: Remote track disable works while disconnect
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Bob disables audio
    And Alice restores WS connection
    Then Alice's audio remote track from Bob is disabled

  Scenario: Local track disable works while disconnect
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Alice disables audio
    And Alice restores WS connection
    Then Bob's audio remote track from Alice is disabled

  Scenario: Disable/enable works fine while disconnect
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Alice disables audio
    And Alice enables audio
    And Alice restores WS connection
    Then Bob's audio remote track from Alice is enabled

  Scenario: Audio endpoint added while disconnected
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Alice loses WS connection
    And Control API interconnects audio of Alice and Bob
    And Alice restores WS connection
    Then Alice has audio remote tracks from Bob
    And Bob has audio remote tracks from Alice

  Scenario: Video endpoint added while disconnected
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Alice loses WS connection
    And Control API interconnects video of Alice and Bob
    And Alice restores WS connection
    Then Alice has video remote tracks from Bob
    And Bob has video remote tracks from Alice

  Scenario: New endpoint creates new tracks
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Alice loses WS connection
    And Control API interconnects Alice and Bob
    And Alice restores WS connection
    Then Alice has audio and video remote tracks from Bob
    And Bob has audio and video remote tracks from Alice

  Scenario: New member joins while disconnected
    Given room with joined member Alice
    And member Bob
    When Alice loses WS connection
    And Bob joins the room
    And Alice restores WS connection
    Then Alice receives connection with Bob
    And Bob receives connection with Alice

  Scenario: `Connection.on_close()` fires when other member leaves while disconnected
    Given room with joined members Alice and Bob
    When Alice loses WS connection
    And Bob's room closed by client
    And Alice restores WS connection
    Then Alice's connection with Bob closes

  Scenario: `Connection.on_close()` fires when other member is deleted by Control API while disconnected
    Given room with joined members Alice and Bob
    When Alice loses WS connection
    And Control API removes member Bob
    And Alice restores WS connection
    Then Alice's connection with Bob closes

  Scenario: Control API deletes WebRtcPublishEndpoint
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Control API deletes Alice's publish endpoint
    And Alice restores WS connection
    Then Bob's remote tracks from Alice are ended

  Scenario: Control API deletes WebRtcPlayEndpoint
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Control API deletes Alice's play endpoint with Bob
    And Alice restores WS connection
    Then Alice's remote tracks from Bob are ended

  Scenario: Control API deletes all endpoints
    Given room with joined member Alice and Bob
    When Alice loses WS connection
    And Control API deletes Alice's publish endpoint
    And Control API deletes Alice's play endpoint with Bob
    And Alice restores WS connection
    Then Alice's connection with Bob closes
    And Bob's connection with Alice closes

  Scenario: Create and delete endpoints while disconnected
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Alice loses WS connection
    And Control API starts Alice's audio publishing to Bob
    And Control API deletes Alice's publish endpoint
    And Alice restores WS connection
    Then Alice doesn't have live local tracks
