Feature: Send Media disabling

  Scenario: Member disables video while call
    Given joined Member `Alice`
    And joined Member `Bob`
    When Member `Bob` disables video
    Then `Alice`'s device video RemoteMediaTrack with `Bob` is disabled
    And `Alice`'s audio RemoteMediaTrack with `Bob` is enabled

  Scenario: Member disables audio while call
    Given joined Member `Alice`
    And joined Member `Bob`
    When Member `Bob` disables audio
    Then `Alice`'s audio RemoteMediaTrack with `Bob` is disabled
    And `Alice`'s device video RemoteMediaTrack with `Bob` is enabled

  Scenario: Member disables video before call
    Given joined Member `Alice`
    And Member `Bob` with disabled video
    When `Bob` joins Room
    Then `Alice` doesn't have device video RemoteMediaTrack with `Bob`
    And `Alice`'s audio RemoteMediaTrack with `Bob` is enabled

  Scenario: Member disables audio before call
    Given joined Member `Alice`
    And Member `Bob` with disabled audio
    When `Bob` joins Room
    Then `Alice` doesn't have audio RemoteMediaTrack with `Bob`
    And `Alice`'s device video RemoteMediaTrack with `Bob` is enabled

  Scenario: Member enables audio while call
    Given joined Member `Alice`
    And Member `Bob` with disabled audio
    When `Bob` joins Room
    And Member `Bob` enables audio
    Then `Alice`'s audio RemoteMediaTrack with `Bob` is enabled

  Scenario: Member enables video while call
    Given joined Member `Alice`
    And Member `Bob` with disabled video
    When `Bob` joins Room
    And Member `Bob` enables video
    Then `Alice`'s device video RemoteMediaTrack with `Bob` is enabled


