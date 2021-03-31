Feature: Delete endpoint

  Scenario: Control API deletes WebRtcPublishEndpoint
    Given room with joined member Alice and Bob
    When Control API deletes Alice's publish endpoint
    Then Bob's remote tracks from Alice are ended

  Scenario: Control API deletes WebRtcPlayEndpoint
    Given room with joined member Alice and Bob
    When Control API deletes Alice's play endpoint with Bob
    Then Alice's remote tracks from Bob are ended

  Scenario: Control API deletes all endpoints
    Given room with joined member Alice and Bob
    When Control API deletes Alice's publish endpoint
    And Control API deletes Alice's play endpoint with Bob
    Then Alice's connection with Bob closes
    And Bob's connection with Alice closes

  Scenario: Publishing continues when WebRtcPlayEndpoint is deleted
    Given room with joined member Alice and Bob
    When Control API deletes Alice's play endpoint with Bob
    Then Bob's remote tracks from Alice are live

  Scenario: Publishing continues when partner's WebRtcPublishEndpoint is deleted
    Given room with joined member Alice and Bob
    When Control API deletes Alice's publish endpoint
    Then Alice's remote tracks from Bob are live
