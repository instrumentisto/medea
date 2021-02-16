Feature: Enable remote media

  Scenario: on_enabled fires on video enable
    Given joined Member `Bob`
    Given joined Member `Alice` with disabled remote video
    When Member `Alice` enables remote video
    Then on_enabled callback fires 1 time on `Alice`'s remote device video Track from `Bob`

  Scenario: on_enabled fires on audio enable
    Given joined Member `Bob`
    Given joined Member `Alice` with disabled remote audio
    When Member `Alice` enables remote audio
    Then on_enabled callback fires 1 time on `Alice`'s remote audio Track from `Bob`

  Scenario: on_enabled doesn't fires on Track create
    Given joined Member `Alice`
    And Member `Bob`
    When `Bob` joins Room
    Then on_enabled callback fires 0 time on `Alice`'s remote audio Track from `Bob`
    And on_enabled callback fires 0 time on `Bob`'s remote audio Track from `Alice`
    And on_enabled callback fires 0 time on `Alice`'s remote device video Track from `Bob`
    And on_enabled callback fires 0 time on `Bob`'s remote device video Track from `Alice`

  Scenario: Remote Member enables video
    Given joined Member `Alice`
    And joined Member `Bob` with disabled local video
    When Member `Bob` enables video
    Then on_enabled callback fires 1 time on `Alice`'s remote device video Track from `Bob`

  Scenario: Remote Member enables audio
    Given joined Member `Alice`
    And joined Member `Bob` with disabled local audio
    When Member `Bob` enables audio
    Then on_enabled callback fires 1 time on `Alice`'s remote audio Track from `Bob`
