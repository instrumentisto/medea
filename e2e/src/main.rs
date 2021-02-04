#![allow(dead_code)]

mod browser;
mod conf;
mod control;
mod entity;
mod file_server;
mod model;
mod world;

use cucumber_rust::{given, then, when, WorldInit as _};
use medea_control_api_mock::proto;

use self::{file_server::FileServer, world::BrowserWorld};
use crate::{entity::room::MediaKind, model::member::Member};

#[given(regex = "(joined )?(send-only |receive-only |empty )?Member `(.*)`( \
                 with (disabled|muted) (audio|video))?")]
async fn given_member(
    world: &mut BrowserWorld,
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

    let member = Member::new(id.clone(), is_send, is_recv);
    world.create_member(member).await;
    if is_joined {
        world.join_room(&id).await;
        world.wait_for_interconnection(&id).await;
    }

    let member = world.get_member(&id);
    if !mute_disable.is_empty() {
        if disabled_or_muted.contains("disabled") {
            let kind = if audio_or_video.contains("audio") {
                MediaKind::Audio
            } else {
                MediaKind::Video
            };
            member.disable_media(kind, None).await;
        }
    }
}

#[when(regex = "Member `(.*)` (disables|mutes) (audio|video)")]
async fn when_disables_mutes(
    world: &mut BrowserWorld,
    id: String,
    disable_or_mutes: String,
    audio_or_video: String,
) {
    let member = world.get_member(&id);
    if disable_or_mutes == "disables" {
        let kind = if audio_or_video.contains("audio") {
            MediaKind::Audio
        } else {
            MediaKind::Video
        };
        member.disable_media(kind, None).await;
    } else {
        todo!()
    }
}

#[tokio::main]
async fn main() {
    let _server = FileServer::run();
    let runner = BrowserWorld::init(&["./features"]);
    runner.run_and_exit().await;
}
