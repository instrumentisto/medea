Feature: Delete endpoint

  Scenario: Control API deletes WebRtcPublishEndpoint
    Given room with joined member Alice and Bob
    When Control API deletes Alice's publish endpoint
    Then Bob has 2 stopped remote tracks from Alice

  Scenario: Control API deletes WebRtcPlayEndpoint
    Given room with joined member Alice and Bob
    When Control API deletes Alice's play endpoint with Bob
    Then Alice has 2 stopped remote tracks from Bob

  Scenario: Control API deletes all endpoints
    Given room with joined member Alice and Bob
    When Control API deletes Alice's publish endpoint
    And Control API deletes Alice's play endpoint with Bob
    Then Alice's connection with Bob closes
    And Bob's connection with Alice closes

  Scenario: Publishing continues when WebRtcPlayEndpoint is deleted
    Given room with joined member Alice and Bob
    When Control API deletes Alice's play endpoint with Bob
    Then Bob has 2 live remote tracks from Alice

  Scenario: Publishing continues when partner's WebRtcPublishEndpoint is deleted
    Given room with joined member Alice and Bob
    When Control API deletes Alice's publish endpoint
    Then Alice has 2 live remote tracks from Bob
