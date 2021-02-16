#![allow(clippy::module_name_repetitions)]

mod browser;
mod conf;
mod control;
mod file_server;
mod object;
mod then;
mod when;
mod world;

use cucumber_rust::{given, WorldInit as _};
use tokio_1 as tokio;

use self::{
    file_server::FileServer,
    object::room::{FailedParsing, MediaKind, MediaSourceKind},
    world::World,
};
use crate::world::MemberBuilder;

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
