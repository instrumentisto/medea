Feature: Media send muting

  Scenario: Member mutes video before call and track is created and enabled
    Given joined Member `Alice`
    And Member `Bob` with muted video
    When `Bob` joins Room
    Then `Alice`'s device video RemoteMediaTrack with `Bob` is enabled

  Scenario: Member mutes audio before call and track is created and enabled
    Given joined Member `Alice`
    And Member `Bob` with muted audio
    When `Bob` joins Room
    Then `Alice`'s audio RemoteMediaTrack with `Bob` is enabled

  Scenario: Member mutes audio before call
    Given joined Member `Alice`
    And Member `Bob` with muted audio
    When `Bob` joins Room
    Then `Alice`'s audio RemoteMediaTrack with `Bob` is muted

  Scenario: Member mutes video before call
    Given joined Member `Alice`
    And Member `Bob` with muted video
    When `Bob` joins Room
    Then `Alice`'s device video RemoteMediaTrack with `Bob` is muted

  Scenario: Member mutes audio while call
    Given joined Member `Alice`
    And joined Member `Bob`
    When Member `Bob` mutes video
    Then `Alice`'s device video RemoteMediaTrack with `Bob` is muted

  Scenario: Member mutes audio while call
    Given joined Member `Alice`
    And joined Member `Bob`
    When Member `Bob` mutes audio
    Then `Alice`'s audio RemoteMediaTrack with `Bob` is muted

#  Scenario: Member mutes video before call and unmutes while call
#    Given joined Member `Alice`
#    And Member `Bob` with muted video
#    When `Bob` joins Room
#    And Member `Bob` unmutes video
#    Then `Alice`'s device video RemoteMediaTrack with `Bob` is unmuted
#
#  Scenario: Member mutes audio before call and unmutes while call
#    Given joined Member `Alice`
#    And Member `Bob` with muted audio
#    When `Bob` joins Room
#    And Member `Bob` unmutes audio
#    Then `Alice`'s audio RemoteMediaTrack with `Bob` is unmuted
#
#  Scenario: Member mutes and unmutes video while call
#    Given joined Member `Alice`
#    And joined Member `Bob`
#    When Member `Bob` mutes video
#    And Member `Bob` unmutes video
#    Then `Alice`'s device video RemoteMediaTrack with `Bob` is unmuted
#
#  Scenario: Member mutes and unmutes video while call
#    Given joined Member `Alice`
#    And joined Member `Bob`
#    When Member `Bob` mutes audio
#    And Member `Bob` unmutes audio
#    Then `Alice`'s audio RemoteMediaTrack with `Bob` is unmuted
