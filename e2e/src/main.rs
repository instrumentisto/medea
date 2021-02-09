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
use crate::object::room::MediaSourceKind;

#[tokio::main]
async fn main() {
    let _server = FileServer::run();
    let runner = World::init(&[conf::FEATURES_PATH.as_str()]);
    runner.run_and_exit().await;
}

fn parse_media_kind(text: &str) -> Option<MediaKind> {
    match text {
        "audio" => Some(MediaKind::Audio),
        "video" => Some(MediaKind::Video),
        "all" => None,
        _ => unreachable!(),
    }
}

#[given(regex = "^(joined )?(send-only |receive-only |empty )?Member `(.*)`( \
                 with (disabled|muted) (audio|video|all))?$")]
async fn given_member(
    world: &mut World,
    joined: String,
    direction: String,
    id: String,
    mute_disable: String,
    disabled_or_muted: String,
    audio_or_video: String,
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

    let member = world.get_member(&id).unwrap();
    if !mute_disable.is_empty() {
        match disabled_or_muted.as_str() {
            "disabled" => {
                if let Some(kind) = parse_media_kind(&audio_or_video) {
                    member.disable_media(kind, None).await.unwrap();
                } else {
                    member.disable_media(MediaKind::Audio, None).await.unwrap();
                    member.disable_media(MediaKind::Video, None).await.unwrap();
                }
            }
            "muted" => {
                if let Some(kind) = parse_media_kind(&audio_or_video) {
                    member.mute_media(kind, None).await.unwrap();
                } else {
                    member.mute_media(MediaKind::Audio, None).await.unwrap();
                    member.mute_media(MediaKind::Video, None).await.unwrap();
                }
            },
            _ => unreachable!(),
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
        if let Some(kind) = parse_media_kind(&audio_or_video) {
            member.disable_media(kind, None).await.unwrap();
        } else {
            member.disable_media(MediaKind::Audio, None).await.unwrap();
            member.disable_media(MediaKind::Video, None).await.unwrap();
        }
    } else {
        if let Some(kind) = parse_media_kind(&audio_or_video) {
            member.mute_media(kind, None).await.unwrap();
        } else {
            member.mute_media(MediaKind::Audio, None).await.unwrap();
            member.mute_media(MediaKind::Video, None).await.unwrap();
        }
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
        if let Some(kind) = parse_media_kind(&audio_or_video) {
            member.enable_media(kind, None).await.unwrap();
        } else {
            member.enable_media(MediaKind::Audio, None).await.unwrap();
            member.enable_media(MediaKind::Video, None).await.unwrap();
        }
    } else {
        if let Some(kind) = parse_media_kind(&audio_or_video) {
            member.unmute_media(kind, None).await.unwrap()
        } else {
            member.unmute_media(MediaKind::Audio, None).await.unwrap();
            member.unmute_media(MediaKind::Video, None).await.unwrap();
        }
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

#[then(regex = "^`(.*)`'s (audio|(display|device) video) RemoteMediaTrack with `(.*)` is (enabled|disabled|muted|unmuted)$")]
async fn then_remote_media_track(
    world: &mut World,
    id: String,
    kind: String,
    _source: String,
    partner_id: String,
    state: String,
) {
    let member = world.get_member(&id).unwrap();
    let partner_connection = member.connections().wait_for_connection(partner_id).await.unwrap();
    let tracks_with_partner = partner_connection.tracks_store().await;

    let (kind, source_kind) = match kind.as_str() {
        "audio" => (MediaKind::Audio, MediaSourceKind::Device),
        "display video" => (MediaKind::Video, MediaSourceKind::Display),
        "device video" => (MediaKind::Video, MediaSourceKind::Device),
        _ => unreachable!()
    };
    let track = tracks_with_partner.get_track(kind, source_kind).await;

    let check = match state.as_str() {
        "enabled" => track.enabled().await,
        "disabled" => !track.enabled().await,
        "muted" => track.muted().await,
        "unmuted" => !track.muted().await,
        _ => unreachable!()
    };
    assert!(check, "RemoteMediaTrack isn't {}", state);
}

#[then(regex = "^`(.*)` doesn't have (audio|(device|display) video) RemoteMediaTrack with `(.*)`$")]
async fn then_doesnt_have_remote_track(
    world: &mut World,
    id: String,
    kind: String,
    _source: String,
    partner_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let partner_connection = member.connections().wait_for_connection(partner_id).await.unwrap();
    let tracks_with_partner = partner_connection.tracks_store().await;

    let (kind, source_kind) = match kind.as_str() {
        "audio" => (MediaKind::Audio, MediaSourceKind::Device),
        "display video" => (MediaKind::Video, MediaSourceKind::Display),
        "device video" => (MediaKind::Video, MediaSourceKind::Device),
        _ => unreachable!()
    };

    assert!(!tracks_with_partner.has_track(kind, source_kind).await);
}