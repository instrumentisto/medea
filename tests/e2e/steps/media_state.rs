use cucumber_rust::{then, when};

use crate::{object::AwaitCompletion, world::World};

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

#[when(regex = "^(\\S+) (enables|disables|mutes|unmutes) (audio|video)\
                 ( and awaits it (complete|error)s)?$")]
async fn when_enables_or_mutes(
    world: &mut World,
    id: String,
    action: String,
    audio_or_video: String,
    awaits: String,
) {
    let member = world.get_member(&id).unwrap();
    let maybe_await = if awaits.is_empty() {
        AwaitCompletion::Dont
    } else {
        AwaitCompletion::Do
    };

    let result = match action.as_str() {
        "enables" => {
            member
                .toggle_media(
                    parse_media_kind(&audio_or_video),
                    None,
                    true,
                    maybe_await,
                )
                .await
        }
        "disables" => {
            member
                .toggle_media(
                    parse_media_kind(&audio_or_video),
                    None,
                    false,
                    maybe_await,
                )
                .await
        }
        "mutes" => {
            member
                .toggle_mute(
                    parse_media_kind(&audio_or_video),
                    None,
                    true,
                    maybe_await,
                )
                .await
        }
        _ => {
            member
                .toggle_mute(
                    parse_media_kind(&audio_or_video),
                    None,
                    false,
                    maybe_await,
                )
                .await
        }
    };

    if maybe_await == AwaitCompletion::Do {
        if awaits.contains("error") {
            result.unwrap_err();
        } else {
            result.unwrap();
        }
    }
}

#[when(regex = "^(\\S+) (enables|disables) remote \
                 (audio|(?:device |display )?video)$")]
async fn when_member_enables_remote_track(
    world: &mut World,
    id: String,
    toggle: String,
    kind: String,
) {
    let member = world.get_member(&id).unwrap();
    let media_kind = kind.parse().unwrap();
    let source_kind = kind.parse().ok();

    if toggle == "enables" {
        member
            .room()
            .enable_remote_media(media_kind, source_kind)
            .await
            .unwrap();
    } else {
        member
            .room()
            .disable_remote_media(media_kind, source_kind)
            .await
            .unwrap();
    }
}
