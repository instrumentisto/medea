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
use crate::model::member::Member;

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
    if !mute_disable.is_empty() {
        todo!("Muting/Disabling not implemented");
    }

    let member = Member::new(id, is_send, is_recv);
    world.create_member(member).await;
}

#[tokio::main]
async fn main() {
    let _server = FileServer::run();
    let runner = BrowserWorld::init(&["./features"]);
    runner.run_and_exit().await;
}
