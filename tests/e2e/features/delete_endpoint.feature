Feature: Delete Endpoint

  Scenario: Control API deletes WebRtcPublishEndpoint
    Given room with joined member Alice and Bob
    When Control API deletes Alice's publish endpoint
    Then Bob's remote tracks from Alice are stopped

  Scenario: Control API deletes WebRtcPlayEndpoint
    Given room with joined member Alice and Bob
    When Control API deletes Alice's play endpoint with Bob
    Then Alice's remote tracks from Bob are stopped

  Scenario: Control API deletes all Endpoints
    Given room with joined member Alice and Bob
    When Control API deletes Alice's publish endpoint
    And Control API deletes Alice's play endpoint with Bob
    Then Alice's connection with Bob closes
    And Bob's connection with Alice closes

  Scenario: Publishing continues on play Endpoint delete
    Given room with joined member Alice and Bob
    When Control API deletes Alice's play endpoint with Bob
    Then Bob's remote tracks from Alice are live

  Scenario: Publishing continues on play Endpoint delete
    Given room with joined member Alice and Bob
    When Control API deletes Alice's publish endpoint
    Then Alice's remote tracks from Bob are live
