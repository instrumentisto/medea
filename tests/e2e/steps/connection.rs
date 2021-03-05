use cucumber_rust::then;

use crate::world::World;

#[then(regex = r"^(\S+) receives connection with (\S+)$")]
async fn then_member_receives_connection(
    world: &mut World,
    id: String,
    responder_id: String,
) {
    let member = world.get_member(&id).unwrap();
    assert!(member
        .connections()
        .wait_for_connection(responder_id.clone())
        .await
        .is_ok());
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

#[then(regex = r"^(\S+)'s connection with (\S+) closes$")]
async fn then_connection_closes(
    world: &mut World,
    id: String,
    partner_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let connection =
        member.connections().get(partner_id).await.unwrap().unwrap();
    assert!(connection.wait_for_close().await.is_ok());
}
