Feature: Create endpoint

  Scenario: New endpoint creates new connections
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API interconnects Alice and Bob
    Then Alice receives connection with Bob
    And Bob receives connection with Alice

#  TODO: Enable these tests when race add_remote_track is fixed.
#
#  Scenario: New Endpoint creates new Tracks
#    Given room with joined member Alice and Bob with no WebRTC endpoints
#    When Control API interconnects Alice and Bob
#    Then Alice has audio and video remote tracks from Bob
#    And Bob has audio and video remote tracks from Alice
#
#  Scenario: New Endpoint creates new audio Tracks
#    Given room with joined members Alice and Bob with no WebRTC endpoints
#    When Control API interconnects audio of Alice and Bob
#    Then Alice has local audio
#    And Bob has local audio
#    Then Alice has audio remote tracks from Bob
#    And Bob has audio remote tracks from Alice
#
#  Scenario: New Endpoint creates new video Tracks
#    Given room with joined member Alice and Bob with no WebRTC endpoints
#    When Control API interconnects video of Alice and Bob
#    Then Alice has local device video
#    And Bob has local device video
#    Then Alice has video remote tracks from Bob
#    And Bob has video remote tracks from Alice

  Scenario: Only one member publishes all
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API starts Alice's media publishing to Bob
    Then Alice doesn't have remote tracks from Bob
    And Bob has audio and video remote tracks from Alice

  Scenario: Only one member publishes audio
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API starts Alice's audio publishing to Bob
    Then Alice doesn't have remote tracks from Bob
    And Bob has audio remote track from Alice

  Scenario: Only one member publishes video
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API starts Alice's video publishing to Bob
    Then Alice doesn't have remote tracks from Bob
    And Bob has video remote track from Alice

  Scenario: WebRtcPlayEndpoint created after delete
    Given room with joined member Alice and Bob
    When Control API deletes Alice's play endpoint with Bob
    And Control API starts Bob's media publishing to Alice
    Then Alice has 2 alive remote tracks from Bob

  Scenario: Endpoints created after delete
    Given room with joined member Alice and Bob
    When Control API deletes Bob's publish endpoint
    And Control API starts Bob's media publishing to Alice
    Then Alice has 2 alive remote tracks from Bob
