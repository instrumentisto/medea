Feature: Remote media disabling

  Scenario: Remote video Track stops on disable
    Given joined Member `Alice`
    And joined Member `Bob`
    When  Member `Alice` disables remote video
    Then `Alice` remote device video Track from `Bob` disables

  Scenario: Remote audio Track stops on disable
    Given joined Member `Alice`
    And joined Member `Bob`
    When  Member `Alice` disables remote audio
    Then `Alice` remote audio Track from `Bob` disables

  Scenario: Remote Track.on_disabled fires on disable audio
    Given joined Member `Alice`
    And joined Member `Bob`
    When Member `Alice` disables remote audio
    Then on_disabled callback fires 1 time on `Alice`'s remote audio Track from `Bob`

  Scenario: Remote Track.on_disabled fires on disable video
    Given joined Member `Alice`
    And joined Member `Bob`
    When Member `Alice` disables remote video
    Then on_disabled callback fires 1 time on `Alice`'s remote device video Track from `Bob`

  Scenario: Remote Member disables video
    Given joined Member `Alice`
    And joined Member `Bob`
    When Member `Bob` disables video
    Then on_disabled callback fires 1 time on `Alice`'s remote device video Track from `Bob`

  Scenario: Remote Member disables audio
    Given joined Member `Alice`
    And joined Member `Bob`
    When Member `Bob` disables audio
    Then on_disabled callback fires 1 time on `Alice`'s remote audio Track from `Bob`
