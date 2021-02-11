#![allow(clippy::module_name_repetitions)]

mod browser;
mod conf;
mod control;
mod file_server;
mod object;
mod world;

use cucumber_rust::{given, then, when, WorldInit as _};

use self::{
    file_server::FileServer,
    object::room::MediaKind,
    world::{MemberBuilder, World},
};
use crate::object::room::{FailedParsing, MediaSourceKind};

#[tokio::main]
async fn main() {
    let _server = FileServer::run();
    let runner = World::init(&[conf::FEATURES_PATH.as_str()]);
    runner.run_and_exit().await;
}

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
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
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
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
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
                with `(.*)` is (enabled|disabled|muted|unmuted)$")]
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
    let tracks_with_partner = partner_connection.tracks_store().await;

    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();
    let track = tracks_with_partner.get_track(media_kind, source_kind).await;

    let check = match state.as_str() {
        "enabled" => track.enabled().await,
        "disabled" => !track.enabled().await,
        "muted" => track.muted().await,
        "unmuted" => !track.muted().await,
        _ => unreachable!(),
    };
    assert!(check, "RemoteMediaTrack isn't {}", state);
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
    let tracks_with_partner = partner_connection.tracks_store().await;
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();

    assert!(!tracks_with_partner.has_track(media_kind, source_kind).await);
}

#[when(regex = "^`(.*)`'s Room closed by client$")]
async fn when_room_closed_by_client(world: &mut World, id: String) {
    world.close_room(&id).await;
}

#[then(regex = "^`(.*)`'s Room.on_close callback fires with `(.*)` reason$")]
async fn then_on_close_fires(
    world: &mut World,
    id: String,
    expect_reason: String,
) {
    let reason = world.wait_for_on_close(&id).await;
    assert_eq!(expect_reason, reason);
}

#[when(regex = "^`(.*)`'s Jason object disposes$")]
async fn when_jason_object_disposes(world: &mut World, id: String) {
    world.dispose_jason(&id).await;
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
    connection.wait_for_close().await;
}

#[then(regex = "^Member `(.*)` has (\\d*) local Tracks$")]
async fn then_member_has_local_tracks(
    world: &mut World,
    id: String,
    count: u64,
) {
    let member = world.get_member(&id).unwrap();
    let room = member.room();
    let tracks = room.local_tracks().await;
    assert_eq!(count, tracks.count().await);
}

#[then(regex = "^`(.*)` has local (audio|(?:device |display )?video)$")]
async fn then_has_local_track(world: &mut World, id: String, kind: String) {
    let member = world.get_member(&id).unwrap();
    let room = member.room();
    let tracks = room.local_tracks().await;
    let media_kind = kind.parse().unwrap();
    let source_kind = kind.parse().ok();

    assert!(tracks.has_track(media_kind, source_kind).await)
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
        .get_track(media_kind, source_kind)
        .await;
    assert!(track.muted().await);
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
        .get_track(media_kind, source_kind)
        .await;
    assert_eq!(track.on_disabled_fire_count().await, times);
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
        .get_track(media_kind, source_kind)
        .await;
    assert_eq!(track.on_enabled_fire_count().await, times);
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
        .get_track(media_kind, source_kind)
        .await;
    let muted = muted.as_str() == "muted";
    assert_eq!(muted, track.muted().await);
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
        .get_track(media_kind, source_kind)
        .await
        .free_and_check()
        .await;
    assert!(is_stopped);
}
