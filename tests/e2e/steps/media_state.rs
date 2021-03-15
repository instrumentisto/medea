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

#[when(regex = r"^(\S+) (enables|unmutes) (audio|video|all)$")]
async fn when_enables_mutes(
    world: &mut World,
    id: String,
    disable_or_mutes: String,
    audio_or_video: String,
) {
    let member = world.get_member(&id).unwrap();
    if disable_or_mutes == "enables" {
        member
            .toggle_media(parse_media_kind(&audio_or_video), None, true)
            .await
            .unwrap();
    } else {
        member
            .toggle_mute(parse_media_kind(&audio_or_video), None, false)
            .await
            .unwrap();
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

#[when(regex = "^(\\S*) tries to enable media publishing")]
async fn when_member_tries_to_enable_publishing(world: &mut World, id: String) {
    world
        .get_member(&id)
        .unwrap()
        .toggle_media(None, None, true)
        .await
        .unwrap_err();
}
