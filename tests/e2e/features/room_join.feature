Feature: Room joining
  Scenario: Member joined
    Given room with joined member Alice
    And member Bob
    When Bob joins room
    Then Alice receives connection with Bob
    And Bob receives connection with Alice

  Scenario: Member joined with disabled media
    Given room with member Alice with disabled media publishing
    And joined member Bob
    When Alice joins room
    Then Alice receives connection with Bob
    And Bob receives connection with Alice

  Scenario: Member without Endpoints joined
    Given room with member Alice with no WebRTC endpoints
    And joined member Bob with no WebRTC endpoints
    When Alice joins room
    Then Alice doesn't receives connection with Bob
    And Bob doesn't receives connection with Alice

  Scenario: Third Member joined
    Given room with joined member Alice and Bob
    And member Carol
    When Carol joins room
    Then Alice receives connection with Carol
    And Bob receives connection with Carol