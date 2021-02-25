Feature: Remote media disabling

  Scenario: Remote video Track stops on disable
    Given room with joined member Alice and Bob
    When  Alice disables remote video
    Then Alice remote device video track from Bob disables

  Scenario: Remote audio Track stops on disable
    Given room with joined member Alice and Bob
    When  Alice disables remote audio
    Then Alice remote audio track from Bob disables

  Scenario: Remote Track.on_disabled fires on disable audio
    Given room with joined member Alice and Bob
    When Alice disables remote audio
    Then on_disabled callback fires 1 time on Alice's remote audio track from Bob

  Scenario: Remote Track.on_disabled fires on disable video
    Given room with joined member Alice and Bob
    When Alice disables remote video
    Then on_disabled callback fires 1 time on Alice's remote device video track from Bob

  Scenario: Remote Member disables video
    Given joined member Alice and Bob
    When Bob disables video
    Then on_disabled callback fires 1 time on Alice's remote device video track from Bob

  Scenario: Remote Member disables audio
    Given joined member Alice and Bob
    When Bob disables audio
    Then on_disabled callback fires 1 time on Alice's remote audio track from Bob
