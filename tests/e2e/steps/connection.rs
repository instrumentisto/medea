use cucumber_rust::then;

use crate::world::World;

#[then(regex = "^(\\S*) receives connection with (\\S*)$")]
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

#[then(regex = "^(\\S*) doesn't receives connection with (\\S*)")]
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
