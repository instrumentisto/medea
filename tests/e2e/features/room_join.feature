Feature: Room joining
  Scenario: Member joined
    Given room with joined member Alice
    And member Bob
    When `Bob` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Member joined with disabled media
    Given room with member Alice with disabled media publishing
    And joined member Bob
    When `Alice` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Member without Endpoints joined
    Given room with member Alice with no WebRTC endpoints
    And joined member Bob with no WebRTC endpoints
    When `Alice` joins Room
    Then `Alice` doesn't receives Connection with Member `Bob`
    And `Bob` doesn't receives Connection with Member `Alice`

  Scenario: Third Member joined
    Given room with joined member Alice
    And joined member Bob
    And member Carol
    When `Carol` joins Room
    Then `Alice` receives Connection with Member `Carol`
    And `Bob` receives Connection with Member `Carol`
