mod browser;
mod entity;
mod file_server;
mod world;

use cucumber_rust::{given, then, when, WorldInit as _};

use self::{file_server::FileServer, world::BrowserWorld};

#[given(regex = "Member (.*)")]
async fn given_member(world: &mut BrowserWorld, id: String) {
    world.create_room(&id).await;
}

#[when(regex = "(.*) joins Room")]
async fn when_member_joins_room(world: &mut BrowserWorld, id: String) {
    let room = world.get_room(&id).unwrap();
    room.join(format!(
        "ws://127.0.0.1:8080/ws/test-room/{}?token=test",
        id
    ))
    .await;
}

#[then(regex = "(.*)'s Room.on_new_connection callback fires")]
async fn then_on_new_connection_callback_fires(
    world: &mut BrowserWorld,
    id: String,
) {
    world.wait_for_on_new_connection(&id).await;
}

#[tokio::main]
async fn main() {
    let _server = FileServer::run();
    let runner = BrowserWorld::init(&["./features"]);
    runner.run_and_exit().await;
}
