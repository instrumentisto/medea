use cucumber_rust::when;

use crate::world::World;

#[when(regex = r"^Control API removes member (\S+)$")]
async fn when_control_api_removes_member(world: &mut World, id: String) {
    world.delete_member_element(&id).await;
}

#[when(regex = r"^Control API removes the room$")]
async fn when_control_api_removes_room(world: &mut World) {
    world.delete_room_element().await;
}
