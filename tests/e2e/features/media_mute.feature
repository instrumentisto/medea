Feature: Media send muting

  Scenario: Member mutes video before call and track is created and enabled
    Given room with joined member Alice
    And member Bob with muted video publishing
    When `Bob` joins Room
    Then `Alice`'s device video RemoteMediaTrack with `Bob` is enabled

  Scenario: Member mutes audio before call and track is created and enabled
    Given room with joined member Alice
    And member Bob with muted audio publishing
    When `Bob` joins Room
    Then `Alice`'s audio RemoteMediaTrack with `Bob` is enabled

  Scenario: Local Track doesn't mutes when Member mutes audio before call
    Given room with joined member Alice
    And member Bob with muted audio publishing
    When `Bob` joins Room
    Then `Bob`'s audio local Track is unmuted

  Scenario: Local Track doesn't mutes when Member mutes video before call
    Given room with joined member Alice
    And member Bob with muted video publishing
    When `Bob` joins Room
    Then `Bob`'s device video local Track is unmuted

  Scenario: Local Track doesn't mutes when Member mutes audio while call
    Given room with joined member Alice
    And joined member Bob
    When Member `Bob` mutes video
    Then `Bob`'s device video local Track is unmuted

  Scenario: Local Track doesn't mutes when Member mutes audio while call
    Given room with joined member Alice
    And joined member Bob
    When Member `Bob` mutes audio
    Then `Bob`'s audio local Track is unmuted
