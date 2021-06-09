Feature: Media muting

  Scenario: Member mutes video before call and track is created and enabled
    Given room with joined member Alice
    And member Bob with muted video publishing
    When Bob joins the room
    Then Alice's device video remote track from Bob is enabled

  Scenario: Member mutes audio before call and track is created and enabled
    Given room with joined member Alice
    And member Bob with muted audio publishing
    When Bob joins the room
    Then Alice's audio remote track from Bob is enabled

  Scenario: Local track is not muted when member mutes audio before call
    Given room with joined member Alice
    And member Bob with muted audio publishing
    When Bob joins the room
    Then Bob's audio local track is not muted

  Scenario: Local track is not muted when member mutes video before call
    Given room with joined member Alice
    And member Bob with muted video publishing
    When Bob joins the room
    Then Bob's device video local track is not muted

  Scenario: Local track is not muted when member mutes video during call
    Given room with joined members Alice and Bob
    When Bob mutes video and waits for success
    Then Bob's device video local track is not muted

  Scenario: Local track is not muted when member mutes audio during call
    Given room with joined members Alice and Bob
    When Bob mutes audio and waits for success
    Then Bob's audio local track is not muted

  Scenario: `RemoteTrack.on_muted()` and `RemoteTrack.on_unmuted()` callbacks fire when video is muted/unmuted
    Given room with joined members Alice and Bob
    When Bob mutes video and waits for success
    Then `on_muted` callback fires 1 time on Alice's remote device video track from Bob
    When Bob unmutes video and waits for success
    Then `on_unmuted` callback fires 1 time on Alice's remote device video track from Bob

  Scenario: `RemoteTrack.on_muted()` and `RemoteTrack.on_unmuted()` callbacks fire when audio is muted/unmuted
    Given room with joined members Alice and Bob
    When Bob mutes audio and waits for success
    Then `on_muted` callback fires 1 time on Alice's remote audio track from Bob
    When Bob unmutes audio and waits for success
    Then `on_unmuted` callback fires 1 time on Alice's remote audio track from Bob
