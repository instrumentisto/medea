Feature: Local Track are created

  Scenario: Local Track are created on connect
    Given Member `Alice`
    And joined Member `Bob`
    When `Alice` joins Room
    Then Member `Alice` has 2 local Tracks
    And `Alice` has local device video
    And `Alice` has local audio

  Scenario: Local Tracks are not created when all media disabled
    Given Member `Alice` with disabled local all
    And joined Member `Bob`
    When `Alice` joins Room
    Then Member `Alice` has 0 local Tracks

  Scenario: Local Track creates when Member enables video
    Given joined Member `Alice` with disabled local all
    And joined Member `Bob`
    When Member `Alice` enables video
    Then Member `Alice` has 1 local Tracks
    And `Alice` has local device video

  Scenario: Local Track creates when Member enables audio
    Given joined Member `Alice` with disabled local all
    And joined Member `Bob`
    When Member `Alice` enables audio
    Then Member `Alice` has 1 local Tracks
    And `Alice` has local audio