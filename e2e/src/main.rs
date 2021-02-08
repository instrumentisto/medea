#![allow(clippy::module_name_repetitions)]

mod browser;
mod conf;
mod control;
mod entity;
mod file_server;
mod model;
mod world;

use cucumber_rust::{given, then, when, WorldInit as _};

use crate::{entity::room::MediaKind, model::member::MemberBuilder};

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

    let member = MemberBuilder {
        id: id.clone(),
        is_send,
        is_recv,
    };
    world.create_member(member).await.unwrap();
    if is_joined {
        world.join_room(&id).await.unwrap();
        world.wait_for_interconnection(&id).await.unwrap();
    }

    let member = world.get_member(&id).unwrap();
    if !mute_disable.is_empty() {
        match disabled_or_muted.as_str() {
            "disabled" => {
                let kind = match audio_or_video.as_str() {
                    "audio" => Some(MediaKind::Audio),
                    "video" => Some(MediaKind::Video),
                    "all" => None,
                    _ => unreachable!(),
                };
                if let Some(kind) = kind {
                    member.disable_media(kind, None).await.unwrap();
                } else {
                    member.disable_media(MediaKind::Audio, None).await.unwrap();
                    member.disable_media(MediaKind::Video, None).await.unwrap();
                }
            }
            "muted" => todo!("Muting is unimplemented atm"),
            _ => unreachable!(),
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
    let member = world.get_member(&id).unwrap();
    if disable_or_mutes == "disables" {
        let kind = match audio_or_video.as_str() {
            "audio" => Some(MediaKind::Audio),
            "video" => Some(MediaKind::Video),
            "all" => None,
            _ => unreachable!(),
        };
        if let Some(kind) = kind {
            member.disable_media(kind, None).await.unwrap();
        } else {
            member.disable_media(MediaKind::Audio, None).await.unwrap();
            member.disable_media(MediaKind::Video, None).await.unwrap();
        }
    } else {
        todo!()
    }
}

#[when(regex = "^`(.*)` joins Room")]
async fn when_member_joins_room(world: &mut BrowserWorld, id: String) {
    world.join_room(&id).await.unwrap();
}

#[then(regex = "^`(.*)` receives Connection with Member `(.*)`$")]
async fn then_member_receives_connection(
    world: &mut BrowserWorld,
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
    world: &mut BrowserWorld,
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

#[tokio::main(worker_threads = 1)]
async fn main() {
    let _server = FileServer::run();
    let runner = BrowserWorld::init(&[conf::FEATURES_PATH.as_str()]);
    runner.run_and_exit().await;
}
