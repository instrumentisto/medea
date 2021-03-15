use cucumber_rust::when;

use crate::world::World;

#[when(regex = r"^(\S+) loses WebSocket connection$")]
async fn ws_connection_loss(world: &mut World, id: String) {
    let member = world.get_member(&id).unwrap();
    member.ws_mock().enable_connection_loss(9999).await;
}

#[when(regex = r"^(\S+) restores WebSocket connection$")]
async fn ws_connection_restore(world: &mut World, id: String) {
    let member = world.get_member(&id).unwrap();
    member.ws_mock().disable_connection_loss().await;
}
