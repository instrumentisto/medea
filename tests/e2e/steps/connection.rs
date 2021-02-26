use cucumber_rust::then;

use crate::world::World;

#[then(regex = r"^(\S+) receives connection with (\S+)$")]
async fn then_member_receives_connection(
    world: &mut World,
    id: String,
    responder_id: String,
) {
    let member = world.get_member(&id).unwrap();
    member
        .connections()
        .wait_for_connection(responder_id.clone())
        .await
        .unwrap();
    assert!(true);
}

#[then(regex = r"^(\S+) doesn't receive connection with (\S+)$")]
async fn then_member_doesnt_receive_connection(
    world: &mut World,
    id: String,
    responder_id: String,
) {
    let member = world.get_member(&id).unwrap();
    assert!(member
        .connections()
        .get(responder_id)
        .await
        .unwrap()
        .is_none())
}
