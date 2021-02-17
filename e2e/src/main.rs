#![allow(clippy::module_name_repetitions)]

mod browser;
mod conf;
mod control;
mod file_server;
mod object;
mod world;

use std::time::Duration;

use cucumber_rust::{given, then, when, WorldInit as _};
use tokio::time::timeout;

use crate::world::{MembersPair, PairedMember};

use self::{
    file_server::FileServer,
    object::room::{FailedParsing, MediaKind, MediaSourceKind},
    world::{MemberBuilder, World},
};
use medea_control_api_mock::proto::{AudioSettings, VideoSettings};

#[tokio::main]
async fn main() {
    let _server = FileServer::run();
    let runner = World::init(&[conf::FEATURES_PATH.as_str()]);
    runner.run_and_exit().await;
}

/// Tries to find `audio`, `video` or `all` in the provided text. If `audio` or
/// `video` found, then [`Some`] [`MediaKind`] will be returned. If `all` found,
/// the [`None`] will be returned. Otherwise this function will panic.
fn parse_media_kind(text: &str) -> Option<MediaKind> {
    if text.contains("audio") {
        Some(MediaKind::Audio)
    } else if text.contains("video") {
        Some(MediaKind::Video)
    } else if text.contains("all") {
        None
    } else {
        unreachable!()
    }
}

/// Parses [`MediaKind`] and [`MediaSourceKind`] from the provided [`str`].
fn parse_media_kinds(
    s: &str,
) -> Result<(MediaKind, MediaSourceKind), FailedParsing> {
    let media_kind = s.parse()?;
    let source_kind = match media_kind {
        MediaKind::Audio => MediaSourceKind::Device,
        MediaKind::Video => s.parse()?,
    };

    Ok((media_kind, source_kind))
}

#[given(regex = "^(joined )?(send-only |receive-only |empty )?Member `(.*)`( \
                 with (?:disabled|muted)(?: remote| local)? \
                 (?:audio|video|all))?$")]
async fn given_member_new(
    world: &mut World,
    joined: String,
    direction: String,
    id: String,
    media_state: String,
) {
    let is_joined = !joined.is_empty();
    let (is_send, is_recv) = if direction.is_empty() {
        (true, true)
    } else {
        (
            direction.contains("send-only"),
            direction.contains("receive-only"),
        )
    };

    let member_builder = MemberBuilder {
        id: id.clone(),
        is_send,
        is_recv,
    };
    world.create_member(member_builder).await.unwrap();
    if is_joined {
        world.join_room(&id).await.unwrap();
        world.wait_for_interconnection(&id).await.unwrap();
    }

    if !media_state.is_empty() {
        let member = world.get_member(&id).unwrap();
        let media_kind = parse_media_kind(&media_state);
        if media_state.contains("local") {
            if media_state.contains("muted") {
                member.toggle_mute(media_kind, None, true).await.unwrap();
            } else if media_state.contains("disabled") {
                member.toggle_media(media_kind, None, false).await.unwrap();
            } else {
                unreachable!()
            }
        } else if media_state.contains("remote") {
            if media_state.contains("disabled") {
                member
                    .toggle_remote_media(media_kind, None, false)
                    .await
                    .unwrap();
            } else {
                unreachable!()
            }
        } else {
            unreachable!()
        }
    }
}

#[when(regex = "^Member `(.*)` (disables|mutes) (audio|video|all)$")]
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

#[when(regex = "^Member `(.*)` (enables|unmutes) (audio|video|all)$")]
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

#[when(regex = "^`(.*)` joins Room")]
async fn when_member_joins_room(world: &mut World, id: String) {
    world.join_room(&id).await.unwrap();
    world.wait_for_interconnection(&id).await.unwrap();
}

#[then(regex = "^`(.*)` receives Connection with Member `(.*)`$")]
async fn then_member_receives_connection(
    world: &mut World,
    id: String,
    partner_id: String,
) {
    let member = world.get_member(&id).unwrap();
    member
        .connections()
        .wait_for_connection(partner_id.clone())
        .await
        .unwrap();
}

#[then(regex = "^`(.*)` doesn't receives Connection with Member `(.*)`")]
async fn then_member_doesnt_receives_connection(
    world: &mut World,
    id: String,
    partner_id: String,
) {
    let member = world.get_member(&id).unwrap();
    assert!(member
        .connections()
        .get(partner_id)
        .await
        .unwrap()
        .is_none())
}

#[then(regex = "^`(.*)`'s (audio|(?:display|device) video) RemoteMediaTrack \
                with `(.*)` is (enabled|disabled)$")]
async fn then_remote_media_track(
    world: &mut World,
    id: String,
    kind: String,
    partner_id: String,
    state: String,
) {
    let member = world.get_member(&id).unwrap();
    let partner_connection = member
        .connections()
        .wait_for_connection(partner_id)
        .await
        .unwrap();
    let tracks_with_partner = partner_connection.tracks_store().await.unwrap();

    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();
    let track = tracks_with_partner
        .get_track(media_kind, source_kind)
        .await
        .unwrap();

    match state.as_str() {
        "enabled" => track.wait_for_enabled().await.unwrap(),
        "disabled" => track.wait_for_disabled().await.unwrap(),
        _ => unreachable!(),
    };
}

#[then(regex = "^`(.*)` doesn't have (audio|(?:device|display) video) \
                RemoteMediaTrack with `(.*)`$")]
async fn then_doesnt_have_remote_track(
    world: &mut World,
    id: String,
    kind: String,
    partner_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let partner_connection = member
        .connections()
        .wait_for_connection(partner_id)
        .await
        .unwrap();
    let tracks_with_partner = partner_connection.tracks_store().await.unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();

    assert!(!tracks_with_partner
        .has_track(media_kind, Some(source_kind))
        .await
        .unwrap());
}

#[when(regex = "^`(.*)`'s Room closed by client$")]
async fn when_room_closed_by_client(world: &mut World, id: String) {
    world.close_room(&id).await.unwrap();
}

#[then(regex = "^`(.*)`'s Room.on_close callback fires with `(.*)` reason$")]
async fn then_on_close_fires(
    world: &mut World,
    id: String,
    expect_reason: String,
) {
    let reason = world.wait_for_on_close(&id).await.unwrap();
    assert_eq!(expect_reason, reason);
}

#[when(regex = "^`(.*)`'s Jason object disposes$")]
async fn when_jason_object_disposes(world: &mut World, id: String) {
    world.dispose_jason(&id).await.unwrap();
}

#[when(regex = "^Control API removes Member `(.*)`$")]
async fn when_control_api_removes_member(world: &mut World, id: String) {
    world.delete_member_element(&id).await;
}

#[when(regex = "^Control API removes Room$")]
async fn when_control_api_removes_room(world: &mut World) {
    world.delete_room_element().await;
}

#[then(regex = "^`(.*)`'s Connection with `(.*)` closes$")]
async fn then_connection_closes(
    world: &mut World,
    id: String,
    partner_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let connection =
        member.connections().get(partner_id).await.unwrap().unwrap();
    connection.wait_for_close().await.unwrap();
}

#[then(regex = "^Member `(.*)` has (\\d*) local Tracks$")]
async fn then_member_has_local_tracks(
    world: &mut World,
    id: String,
    count: u64,
) {
    let member = world.get_member(&id).unwrap();
    let room = member.room();
    let tracks = room.local_tracks().await.unwrap();
    assert_eq!(count, tracks.count().await.unwrap());
}

#[then(regex = "^`(.*)` has local (audio|(?:device |display )?video)$")]
async fn then_has_local_track(world: &mut World, id: String, kind: String) {
    let member = world.get_member(&id).unwrap();
    let room = member.room();
    let tracks = room.local_tracks().await.unwrap();
    let media_kind = kind.parse().unwrap();
    // let source_kind = kind.parse().ok();

    tracks.get_track(media_kind, MediaSourceKind::Device).await.unwrap();
}

#[when(
    regex = "Member `(.*)` enables remote (audio|(?:device |display )?video)"
)]
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

#[when(regex = "^Member `(.*)` disables remote (audio|(?:device |display \
                )?video)$")]
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

#[then(regex = "^`(.*)` remote (audio|(?:device|display) video) Track from \
                `(.*)` disables")]
async fn then_remote_track_stops(
    world: &mut World,
    id: String,
    kind: String,
    remote_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();

    let conn = member.connections().get(remote_id).await.unwrap().unwrap();
    let track = conn
        .tracks_store()
        .await
        .unwrap()
        .get_track(media_kind, source_kind)
        .await
        .unwrap();
    assert!(track.muted().await.unwrap());
}

#[then(regex = "^on_disabled callback fires (\\d*) time on `(.*)`'s remote \
                (audio|(?:device|display) video) Track from `(.*)`$")]
async fn then_on_remote_disabled_callback_fires(
    world: &mut World,
    times: u64,
    id: String,
    kind: String,
    remote_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let remote_conn =
        member.connections().get(remote_id).await.unwrap().unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();

    let track = remote_conn
        .tracks_store()
        .await
        .unwrap()
        .get_track(media_kind, source_kind)
        .await
        .unwrap();
    track.wait_for_on_disabled_fire_count(times).await.unwrap();
}

#[then(regex = "^on_enabled callback fires (\\d*) time on `(.*)`'s remote \
                (audio|(?:device|display) video) Track from `(.*)`$")]
async fn then_on_remote_enabled_callback_fires(
    world: &mut World,
    times: u64,
    id: String,
    kind: String,
    remote_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let remote_conn =
        member.connections().get(remote_id).await.unwrap().unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();
    let track = remote_conn
        .tracks_store()
        .await
        .unwrap()
        .get_track(media_kind, source_kind)
        .await
        .unwrap();
    track.wait_for_on_enabled_fire_count(times).await.unwrap();
}

#[then(regex = "^`(.*)`'s (audio|(?:device|display) video) local Track is \
                (muted|unmuted)$")]
async fn then_local_track_mute_state(
    world: &mut World,
    id: String,
    kind: String,
    muted: String,
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
    let muted = muted.as_str() == "muted";
    assert_eq!(muted, track.muted().await.unwrap());
}

#[then(regex = "^`(.*)`'s (audio|(?:device|display) video) local Track is \
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

#[then(regex = "^Control API sends OnLeave callback with `(.*)` reason for \
                Member `(.*)`$")]
async fn then_control_api_sends_on_leave(
    world: &mut World,
    reason: String,
    id: String,
) {
    timeout(Duration::from_secs(10), world.wait_for_on_leave(id, reason))
        .await
        .unwrap();
}

#[rustfmt::skip]
#[then(
    regex = "^Control API doesn't sends OnLeave callback for Member `(.*)`$"
)]
async fn then_control_api_doesnt_sends_on_leave(world: &mut World, id: String) {
    timeout(
        Duration::from_millis(300),
        world.wait_for_on_leave(id, "".to_string()),
    )
    .await
    .unwrap_err();
}

#[then(regex = "^Control API sends OnJoin callback for Member `(.*)`$")]
async fn then_control_api_sends_on_join(world: &mut World, id: String) {
    timeout(Duration::from_secs(10), world.wait_for_on_join(id))
        .await
        .unwrap();
}

#[when(regex = "^Control API interconnects `(.*)` and `(.*)`$")]
async fn when_control_api_interconnects_members(
    world: &mut World,
    id: String,
    partner_id: String,
) {
    world
        .interconnect_members(MembersPair {
            left: PairedMember {
                id,
                recv: true,
                send_video: Some(VideoSettings::default()),
                send_audio: Some(AudioSettings::default()),
            },
            right: PairedMember {
                id: partner_id,
                recv: true,
                send_video: Some(VideoSettings::default()),
                send_audio: Some(AudioSettings::default()),
            },
        })
        .await
        .unwrap();
}

#[then(
    regex = "^`(.*)` has (audio|video|audio and video) remote Track(?:s)? \
             with `(.*)`"
)]
async fn then_member_has_remote_track(
    world: &mut World,
    id: String,
    kind: String,
    remote_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let connection = member
        .connections()
        .wait_for_connection(remote_id)
        .await
        .unwrap();
    let tracks_store = connection.tracks_store().await.unwrap();

    if kind.contains("audio") {
        tracks_store
            .get_track(MediaKind::Audio, MediaSourceKind::Device)
            .await
            .unwrap();
    }
    if kind.contains("video") {
        tracks_store
            .get_track(MediaKind::Video, MediaSourceKind::Device)
            .await
            .unwrap();
    }
}

#[when(
    regex = "^Control API interconnected (audio|video) of `(.*)` and `(.*)`$"
)]
async fn when_interconnects_kind(
    world: &mut World,
    kind: String,
    left_member_id: String,
    right_member_id: String,
) {
    let send_video = if kind.contains("video") {
        Some(VideoSettings {
            publish_policy: proto::PublishPolicy::Required
        })
    } else {
        None
    };
    use medea_control_api_mock::proto;
    let send_audio = if kind.contains("audio") {
        Some(AudioSettings {
            publish_policy: proto::PublishPolicy::Required
        })
    } else {
        None
    };

    world
        .interconnect_members(MembersPair {
            left: PairedMember {
                id: left_member_id,
                recv: true,
                send_video: send_video.clone(),
                send_audio: send_audio.clone(),
            },
            right: PairedMember {
                id: right_member_id,
                recv: true,
                send_video,
                send_audio,
            },
        })
        .await
        .unwrap();
}

#[when(regex = "^Control API starts `(.*)`'s (audio|video|media) publishing \
                to `(.*)`$")]
async fn when_control_api_starts_publishing(
    world: &mut World,
    publisher_id: String,
    kind: String,
    receiver_id: String,
) {
    let all_kinds = kind.contains("media");
    let send_audio = if all_kinds || kind.contains("audio") {
        Some(AudioSettings::default())
    } else {
        None
    };
    let send_video = if all_kinds || kind.contains("video") {
        Some(VideoSettings::default())
    } else {
        None
    };
    world
        .interconnect_members(MembersPair {
            left: PairedMember {
                id: publisher_id,
                recv: false,
                send_audio,
                send_video,
            },
            right: PairedMember {
                id: receiver_id,
                recv: true,
                send_video: None,
                send_audio: None,
            },
        })
        .await
        .unwrap();
}

#[then(regex = "^`(.*)` doesn't has remote Tracks from `(.*)`$")]
async fn then_member_doesnt_has_remote_tracks_with(
    world: &mut World,
    id: String,
    partner_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let tracks_count = member
        .connections()
        .wait_for_connection(partner_id)
        .await
        .unwrap()
        .tracks_store()
        .await
        .unwrap()
        .count()
        .await
        .unwrap();
    assert_eq!(tracks_count, 0);
}
