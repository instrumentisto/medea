Feature: Create Endpoint

  Scenario: New Endpoint creates new Connections
    Given joined empty Member `Alice`
    And joined empty Member `Bob`
    When Control API interconnects `Alice` and `Bob`
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: New Endpoint creates new Tracks
    Given joined empty Member `Alice`
    And joined empty Member `Bob`
    When Control API interconnects `Alice` and `Bob`
    Then `Alice` has audio and video remote Tracks with `Bob`
    And `Bob` has audio and video remote Tracks with `Alice`

  Scenario: New Endpoint creates new audio Tracks
    Given joined empty Member `Alice`
    And joined empty Member `Bob`
    When Control API interconnected audio of `Alice` and `Bob`
    Then `Alice` has local audio
    And `Bob` has local audio
    Then `Alice` has audio remote Tracks with `Bob`
    And `Bob` has audio remote Tracks with `Alice`

  Scenario: New Endpoint creates new video Tracks
    Given joined empty Member `Alice`
    And joined empty Member `Bob`
    When Control API interconnected video of `Alice` and `Bob`
    Then `Alice` has local video
    And `Bob` has local video
    Then `Alice` has video remote Tracks with `Bob`
    And `Bob` has video remote Tracks with `Alice`

  Scenario: Only one Member publishes
    Given joined empty Member `Alice`
    And joined empty Member `Bob`
    When Control API starts `Alice`'s media publishing to `Bob`
    Then `Alice` doesn't has remote Tracks from `Bob`
    And `Bob` has audio and video remote Tracks with `Alice`
  Scenario: Only one Member publishes audio
    Given joined empty Member `Alice`
    And joined empty Member `Bob`
    When Control API starts `Alice`'s audio publishing to `Bob`
    Then `Alice` doesn't has remote Tracks from `Bob`
    And `Bob` has audio remote Track with `Alice`
  Scenario: Only one Member publishes video
    Given joined empty Member `Alice`
    And joined empty Member `Bob`
    When Control API starts `Alice`'s video publishing to `Bob`
    Then `Alice` doesn't has remote Tracks from `Bob`
    And `Bob` has video remote Track with `Alice`