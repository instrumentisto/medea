use std::time::Duration;

use cucumber_rust::when;

use crate::world::World;

#[when(regex = "^(\\S*) loses WS connection$")]
async fn ws_connection_loss(world: &mut World, id: String) {
    let member = world.get_member(&id).unwrap();
    member.ws_mock().enable_connection_loss(9999).await;
    tokio_1::time::sleep(Duration::from_secs(1)).await;
}

#[when(regex = "^(\\S*) restores WS connection$")]
async fn ws_connection_restore(world: &mut World, id: String) {
    let member = world.get_member(&id).unwrap();
    member.ws_mock().disable_connection_loss().await;
}
