Feature: Create Endpoint

  Scenario: New Endpoint creates new Connections
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API interconnects Alice and Bob
    Then Alice receives connection with Bob
    And Bob receives connection with Alice

  Scenario: New Endpoint creates new Tracks
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API interconnects Alice and Bob
    Then Alice has audio and video remote tracks with Bob
    And Bob has audio and video remote tracks with Alice

  Scenario: New Endpoint creates new audio Tracks
    Given room with joined members Alice and Bob with no WebRTC endpoints
    When Control API interconnected audio of Alice and Bob
    Then Alice has local audio
    And Bob has local audio
    Then Alice has audio remote tracks with Bob
    And Bob has audio remote tracks with Alice

  Scenario: New Endpoint creates new video Tracks
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API interconnected video of Alice and Bob
    Then Alice has local device video
    And Bob has local device video
    Then Alice has video remote tracks with Bob
    And Bob has video remote tracks with Alice

  Scenario: Only one Member publishes
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API starts Alice's media publishing to Bob
    Then Alice doesn't has remote tracks from Bob
    And Bob has audio and video remote tracks with Alice
  Scenario: Only one Member publishes audio
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API starts Alice's audio publishing to Bob
    Then Alice doesn't has remote tracks from Bob
    And Bob has audio remote track with Alice
  Scenario: Only one Member publishes video
    Given room with joined member Alice and Bob with no WebRTC endpoints
    When Control API starts Alice's video publishing to Bob
    Then Alice doesn't has remote tracks from Bob
    And Bob has video remote track with Alice