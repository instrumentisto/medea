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
    object::MediaKind,
    world::{MemberBuilder, World},
};

fn main() {
    tokio_e2e::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let _server = FileServer::run();
            let runner = World::init(&[conf::FEATURES_PATH.as_str()]);
            runner.run_and_exit().await;
        })
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
    }

    let member = world.get_member(&id).unwrap();
    if !mute_disable.is_empty() {
        match disabled_or_muted.as_str() {
            "disabled" => {
                if let Some(kind) = parse_media_kind(&audio_or_video) {
                    member.disable_media_send(kind, None).await.unwrap();
                } else {
                    member
                        .disable_media_send(MediaKind::Audio, None)
                        .await
                        .unwrap();
                    member
                        .disable_media_send(MediaKind::Video, None)
                        .await
                        .unwrap();
                }
            }
            "muted" => todo!("Muting is unimplemented atm"),
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
            member.disable_media_send(kind, None).await.unwrap();
        } else {
            member
                .disable_media_send(MediaKind::Audio, None)
                .await
                .unwrap();
            member
                .disable_media_send(MediaKind::Video, None)
                .await
                .unwrap();
        }
    } else {
        todo!("Muting is unimplemented atm.")
    }
}

#[when(regex = "^`(.*)` joins Room")]
async fn when_member_joins_room(world: &mut World, id: String) {
    world.join_room(&id).await.unwrap();
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
