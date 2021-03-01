use cucumber_rust::{then, when, given};

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

#[given(regex = "^(\\S*)'s gUM (audio |video )?broken$")]
async fn given_member_gum_broken(world: &mut World, id: String, kind: String) {
    let member = world.get_member(&id).unwrap();
    let gum = member.gum_mock();
    let (video, audio) = if kind.is_empty() {
        (true, true)
    } else {
        (kind.contains("video"), kind.contains("audio"))
    };
    gum.broke_gum(video, audio).await;
}

#[when(regex = "^(\\S*) enables (video|audio|video and audio) constraints$")]
async fn when_member_switches_to_kind(world: &mut World, id: String, kind: String) {
    let member = world.get_member(&id).unwrap();
    let video = kind.contains("video");
    let audio = kind.contains("audio");
    let _ = member.room().set_local_media_settings(video, audio).await.unwrap();
}

#[when(regex = "^(\\S*) disables media in constraints$")]
async fn when_member_disabled_media_in_cons(world: &mut World, id: String) {
    let member = world.get_member(&id).unwrap();
    member.room().set_local_media_settings(false, false).await.unwrap();
}


#[then(regex = "^(\\S*)'s Room.on_failed_local_stream fires (\\d*) time(:?s)?$")]
async fn then_room_failed_local_stream_fires(world: &mut World, id: String, times: u64) {
    let member = world.get_member(&id).unwrap();
    member.room().when_failed_local_stream_count(times).await;
}
