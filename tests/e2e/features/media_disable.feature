Feature: Send Media disabling

  Scenario: Member disables video while call
    Given room with joined member Alice
    And joined member Bob
    When Member `Bob` disables video
    Then `Alice`'s device video RemoteMediaTrack with `Bob` is disabled
    And `Alice`'s audio RemoteMediaTrack with `Bob` is enabled

  Scenario: Member disables audio while call
    Given room with joined member Alice
    And joined member Bob
    When Member `Bob` disables audio
    Then `Alice`'s audio RemoteMediaTrack with `Bob` is disabled
    And `Alice`'s device video RemoteMediaTrack with `Bob` is enabled

  Scenario: Member disables video before call
    Given room with joined member Alice
    And member Bob with disabled video publishing
    When `Bob` joins Room
    Then `Alice` doesn't have device video RemoteMediaTrack with `Bob`
    And `Alice`'s audio RemoteMediaTrack with `Bob` is enabled

  Scenario: Member disables audio before call
    Given room with joined member Alice
    And member Bob with disabled audio publishing
    When `Bob` joins Room
    Then `Alice` doesn't have audio RemoteMediaTrack with `Bob`
    And `Alice`'s device video RemoteMediaTrack with `Bob` is enabled

  Scenario: Member enables audio while call
    Given room with joined member Alice
    And member Bob with disabled audio publishing
    When `Bob` joins Room
    And Member `Bob` enables audio
    Then `Alice`'s audio RemoteMediaTrack with `Bob` is enabled

  Scenario: Member enables video while call
    Given room with joined member Alice
    And member Bob with disabled video publishing
    When `Bob` joins Room
    And Member `Bob` enables video
    Then `Alice`'s device video RemoteMediaTrack with `Bob` is enabled

  Scenario: Local Track is dropped on video disable
    Given room with joined member Alice
    And joined member Bob
    When Member `Bob` disables video
    Then `Bob`'s device video local Track is stopped

  Scenario: Local Track is dropped on audio disable
    Given room with joined member Alice
    And joined member Bob
    When Member `Bob` disables audio
    Then `Bob`'s audio local Track is stopped
