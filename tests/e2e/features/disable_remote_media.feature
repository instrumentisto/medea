Feature: Remote media disabling

  Scenario: Remote video track stops when disabled
    Given room with joined members Alice and Bob
    When Alice disables remote video
    Then Alice's remote device video track from Bob disables

  Scenario: Remote audio track stops when disabled
    Given room with joined members Alice and Bob
    When Alice disables remote audio
    Then Alice's remote audio track from Bob disables

  Scenario: `RemoteTrack.on_disabled()` fires when audio is disabled
    Given room with joined members Alice and Bob
    When Alice disables remote audio
    Then `on_disabled` callback fires 1 time on Alice's remote audio track from Bob

  Scenario: `RemoteTrack.on_disabled()` fires when video is disabled
    Given room with joined members Alice and Bob
    When Alice disables remote video
    Then `on_disabled` callback fires 1 time on Alice's remote device video track from Bob

  Scenario: Remote member disables video
    Given room with joined members Alice and Bob
    When Bob disables video and awaits it completes
    Then `on_disabled` callback fires 1 time on Alice's remote device video track from Bob

  Scenario: Remote member disables audio
    Given room with joined members Alice and Bob
    When Bob disables audio and awaits it completes
    Then `on_disabled` callback fires 1 time on Alice's remote audio track from Bob
