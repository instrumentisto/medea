Feature: Local Track are created

  Scenario: Local Track are created on connect
    Given room with member Alice
    And joined member Bob
    When `Alice` joins Room
    Then Member `Alice` has 2 local Tracks
    And `Alice` has local device video
    And `Alice` has local audio

  Scenario: Local Tracks are not created when all media disabled
    Given room with member Alice with disabled media publishing
    And joined member Bob
    When `Alice` joins Room
    Then Member `Alice` has 0 local Tracks

  Scenario: Local Track creates when Member enables video
    Given room with joined member Alice with disabled media publishing
    And joined member Bob
    When Member `Alice` enables video
    Then Member `Alice` has 1 local Tracks
    And `Alice` has local device video

  Scenario: Local Track creates when Member enables audio
    Given joined member Alice with disabled media publishing
    And joined member Bob
    When Member `Alice` enables audio
    Then Member `Alice` has 1 local Tracks
    And `Alice` has local audio