#![allow(dead_code)]

mod browser;
mod conf;
mod control;
mod entity;
mod file_server;
mod graceful_shutdown;
mod model;
mod world;

use cucumber_rust::{given, then, when, WorldInit as _};
use medea_control_api_mock::proto;

use crate::{entity::room::MediaKind, model::member::Member};

use self::{file_server::FileServer, world::BrowserWorld};

#[given(regex = "^(joined )?(send-only |receive-only |empty )?Member `(.*)`( \
                 with (disabled|muted) (audio|video|all))?$")]
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
            let kind = match audio_or_video.as_str() {
                "audio" => Some(MediaKind::Audio),
                "video" => Some(MediaKind::Video),
                "all" => None,
                _ => unreachable!(),
            };
            if let Some(kind) = kind {
                member.disable_media(kind, None).await;
            } else {
                member.disable_media(MediaKind::Audio, None).await;
                member.disable_media(MediaKind::Video, None).await;
            }
        }
    }
}

#[when(regex = "^Member `(.*)` (disables|mutes) (audio|video|all)$")]
async fn when_disables_mutes(
    world: &mut BrowserWorld,
    id: String,
    disable_or_mutes: String,
    audio_or_video: String,
) {
    let member = world.get_member(&id);
    if disable_or_mutes == "disables" {
        let kind = match audio_or_video.as_str() {
            "audio" => Some(MediaKind::Audio),
            "video" => Some(MediaKind::Video),
            "all" => None,
            _ => unreachable!(),
        };
        if let Some(kind) = kind {
            member.disable_media(kind, None).await;
        } else {
            member.disable_media(MediaKind::Audio, None).await;
            member.disable_media(MediaKind::Video, None).await;
        }
    } else {
        todo!()
    }
}

#[when(regex = "^`(.*)` joins Room")]
async fn when_member_joins_room(world: &mut BrowserWorld, id: String) {
    world.join_room(&id).await;
}

#[then(regex = "^`(.*)` receives Connection with Member `(.*)`$")]
async fn then_member_receives_connection(
    world: &mut BrowserWorld,
    id: String,
    partner_id: String,
) {
    let member = world.get_member(&id);
    member
        .connections()
        .wait_for_connection(partner_id.clone())
        .await;
}

#[then(regex = "^`(.*)` doesn't receives Connection with Member `(.*)`")]
async fn then_member_doesnt_receives_connection(
    world: &mut BrowserWorld,
    id: String,
    partner_id: String,
) {
    let member = world.get_member(&id);
    assert!(member.connections().get(partner_id).await.is_none())
}

#[tokio::main]
async fn main() {
    graceful_shutdown::init();

    let _server = FileServer::run();
    let runner = BrowserWorld::init(&["./features"]);
    let res = runner.run().await;

    graceful_shutdown::wait_for_browser_close();

    let exit_code = if res.failed() { 1 } else { 0 };
    std::process::exit(exit_code);
}
