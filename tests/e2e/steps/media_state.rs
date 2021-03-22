use cucumber_rust::{then, when};

use crate::world::World;

use super::{parse_media_kind, parse_media_kinds};

#[then(regex = "^(\\S+)'s (audio|(?:device|display) video) local track is \
                 (not )?muted$")]
async fn then_local_track_mute_state(
    world: &mut World,
    id: String,
    kind: String,
    not_muted: String,
) {
    let member = world.get_member(&id).unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();
    let track = member
        .room()
        .local_tracks()
        .await
        .unwrap()
        .get_track(media_kind, source_kind)
        .await
        .unwrap();
    assert_eq!(not_muted.is_empty(), track.muted().await.unwrap());
}

#[then(regex = "^(\\S+)'s (audio|(?:device|display) video) local track is \
                 stopped$")]
async fn then_track_is_stopped(world: &mut World, id: String, kind: String) {
    let member = world.get_member(&id).unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();
    let is_stopped = member
        .room()
        .local_tracks()
        .await
        .unwrap()
        .get_track(media_kind, source_kind)
        .await
        .unwrap()
        .free_and_check()
        .await
        .unwrap();
    assert!(is_stopped);
}

#[when(regex = r"^(\S+) (disables|mutes) (audio|video|all)$")]
async fn when_disables_mutes(
    world: &mut World,
    id: String,
    disable_or_mutes: String,
    audio_or_video: String,
) {
    let member = world.get_member(&id).unwrap();
    if disable_or_mutes == "disables" {
        member
            .toggle_media(parse_media_kind(&audio_or_video), None, false)
            .await
            .unwrap()
    } else {
        member
            .toggle_mute(parse_media_kind(&audio_or_video), None, true)
            .await
            .unwrap();
    }
}

#[when(regex = "^(\\S+) (enables|unmutes) (audio|video|all)( with error)?$")]
async fn when_enables_or_mutes(
    world: &mut World,
    id: String,
    disable_or_mutes: String,
    audio_or_video: String,
    with_error: String,
) {
    let is_ok = with_error.is_empty();
    let member = world.get_member(&id).unwrap();
    let result = if disable_or_mutes == "enables" {
        member
            .toggle_media(parse_media_kind(&audio_or_video), None, true)
            .await
    } else {
        member
            .toggle_mute(parse_media_kind(&audio_or_video), None, false)
            .await
    };

    if is_ok {
        result.unwrap();
    } else {
        result.unwrap_err();
    }
}

#[when(regex = r"^(\S+) enables remote (audio|(?:device |display )?video)$")]
async fn when_member_enables_remote_track(
    world: &mut World,
    id: String,
    kind: String,
) {
    let member = world.get_member(&id).unwrap();
    let media_kind = kind.parse().unwrap();
    let source_kind = kind.parse().ok();
    member
        .room()
        .enable_remote_media(media_kind, source_kind)
        .await
        .unwrap();
}

#[when(regex = r"^(\S+) disables remote (audio|(?:device |display )?video)$")]
async fn when_member_disables_remote_track(
    world: &mut World,
    id: String,
    kind: String,
) {
    let member = world.get_member(&id).unwrap();
    let media_kind = kind.parse().unwrap();
    let source_kind = kind.parse().ok();
    member
        .room()
        .disable_remote_media(media_kind, source_kind)
        .await
        .unwrap();
}
