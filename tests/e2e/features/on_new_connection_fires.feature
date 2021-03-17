Feature: `on_new_connection` callback

  Scenario: Member joined with enabled media
    Given room with joined member Alice
    And member Bob
    When Bob joins the room
    Then Alice receives connection with Bob
    And Bob receives connection with Alice

  Scenario: Member joined with disabled media
    Given room with member Alice with disabled media publishing
    And joined member Bob
    When Alice joins the room
    Then Alice receives connection with Bob
    And Bob receives connection with Alice

  Scenario: Member joined without WebRTC endpoints
    Given room with member Alice with no WebRTC endpoints
    And joined member Bob with no WebRTC endpoints
    When Alice joins the room
    Then Alice doesn't receive connection with Bob
    And Bob doesn't receive connection with Alice

  Scenario: Third member joined
    Given room with joined members Alice and Bob
    And member Carol
    When Carol joins the room
    Then Alice receives connection with Carol
    And Bob receives connection with Carol
