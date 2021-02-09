Feature: Room joining
  Scenario: Member joined
    Given joined Member `Alice`
    And Member `Bob`
    When `Bob` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Member joined with disabled media
    Given Member `Alice` with disabled all
    And joined Member `Bob`
    When `Alice` joins Room
    Then `Alice` receives Connection with Member `Bob`
    And `Bob` receives Connection with Member `Alice`

  Scenario: Member without Endpoints joined
    Given empty Member `Alice`
    And joined empty Member `Bob`
    When `Alice` joins Room
    Then `Alice` doesn't receives Connection with Member `Bob`
    And `Bob` doesn't receives Connection with Member `Alice`

  Scenario: Third Member joined
    Given joined Member `Alice`
    And joined Member `Bob`
    And Member `Carol`
    When `Carol` joins Room
    Then `Alice` receives Connection with Member `Carol`
    And `Bob` receives Connection with Member `Carol`

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
