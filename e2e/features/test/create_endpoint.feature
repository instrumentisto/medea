Feature: Create Endpoint

  Scenario: New Endpoint creates new audio Tracks
    Given joined empty Member `Alice`
    And joined empty Member `Bob`
    When Control API interconnected audio of `Alice` and `Bob`
    Then `Alice` has local audio
    And `Bob` has local audio
    Then `Alice` has audio remote Tracks with `Bob`
    And `Bob` has audio remote Tracks with `Alice`

