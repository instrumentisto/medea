Feature: Create Endpoint

  Scenario: New Endpoint creates new Tracks
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API interconnects Alice and Bob
    Then Alice has audio and video remote tracks from Bob
    And Bob has audio and video remote tracks from Alice

  Scenario: New Endpoint creates new audio Tracks
    Given room with joined members Alice and Bob with no WebRTC endpoints
    When Control API interconnects audio of Alice and Bob
    Then Alice has local audio
    And Bob has local audio
    Then Alice has audio remote tracks from Bob
    And Bob has audio remote tracks from Alice

#  Scenario: New Endpoint creates new video Tracks
#    Given room with joined member Alice and Bob with no WebRTC endpoints
#    When Control API interconnects video of Alice and Bob
#    Then Alice has local device video
#    And Bob has local device video
#    Then Alice has video remote tracks from Bob
#    And Bob has video remote tracks from Alice
