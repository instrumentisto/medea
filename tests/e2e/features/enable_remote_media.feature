Feature: Enable remote media

  Scenario: on_enabled fires on video enable
    Given room with joined member Bob
    Given joined member Alice with disabled video playing
    When Member `Alice` enables remote video
    Then on_enabled callback fires 1 time on `Alice`'s remote device video Track from `Bob`

  Scenario: on_enabled fires on audio enable
    Given room with joined member Bob
    Given joined member Alice with disabled audio playing
    When Member `Alice` enables remote audio
    Then on_enabled callback fires 1 time on `Alice`'s remote audio Track from `Bob`

  Scenario: on_enabled doesn't fires on Track create
    Given room with joined member Alice
    And member Bob
    When `Bob` joins Room
    Then on_enabled callback fires 0 time on `Alice`'s remote audio Track from `Bob`
    And on_enabled callback fires 0 time on `Bob`'s remote audio Track from `Alice`
    And on_enabled callback fires 0 time on `Alice`'s remote device video Track from `Bob`
    And on_enabled callback fires 0 time on `Bob`'s remote device video Track from `Alice`

  Scenario: Remote Member enables video
    Given room with joined member Alice
    And joined member Bob with disabled video publishing
    When Member `Bob` enables video
    Then on_enabled callback fires 1 time on `Alice`'s remote device video Track from `Bob`

  Scenario: Remote Member enables audio
    Given room with joined member Alice
    And joined member Bob with disabled audio publishing
    When Member `Bob` enables audio
    Then on_enabled callback fires 1 time on `Alice`'s remote audio Track from `Bob`
