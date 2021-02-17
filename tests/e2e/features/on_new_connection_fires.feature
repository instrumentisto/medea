Feature: `on_new_connection` callback

  Scenario: Member joined
    Given joined Member `Alice`
    And Member `Bob`
    When `Bob` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Member joined
    Given room with joined members Alice, Bob and Charlie
    And member Gendalf with no WebRTC endpoints
    When Gendalf joins the room

  Scenario: Member joined with disabled media
    Given Member `Alice` with disabled all
    And joined Member `Bob`
    When `Alice` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Member without Endpoints joined
    Given empty Member `Alice`
    And joined empty Member `Bob`
    When `Alice` joins Room
    Then `Alice` doesn't receives Connection with Member `Bob`
    And `Bob` doesn't receives Connection with Member `Alice`
