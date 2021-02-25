use cucumber_rust::{then, when};

use crate::world::World;

#[then(regex = "^(\\S*)'s Room.on_close callback fires with `(\\S*)` reason$")]
async fn then_on_close_fires(
    world: &mut World,
    id: String,
    expect_reason: String,
) {
    let reason = world.wait_for_on_close(&id).await.unwrap();
    assert_eq!(expect_reason, reason);
}

#[when(regex = "^(\\S*) joins room")]
async fn when_member_joins_room(world: &mut World, id: String) {
    world.join_room(&id).await.unwrap();
    world.wait_for_interconnection(&id).await.unwrap();
}

#[when(regex = "^(\\S*)'s room closed by client$")]
async fn when_room_closed_by_client(world: &mut World, id: String) {
    world.close_room(&id).await.unwrap();
}

#[when(regex = "^(\\S*)'s Jason object disposes$")]
async fn when_jason_object_disposes(world: &mut World, id: String) {
    world.dispose_jason(&id).await.unwrap();
}