use cucumber_rust::{given, then, when};

use crate::world::World;

#[then(regex = "^(\\S+)'s `on_close` room's callback fires with `(\\S+)` \
                 reason$")]
async fn then_on_close_fires(
    world: &mut World,
    id: String,
    expect_reason: String,
) {
    let reason = world.wait_for_on_close(&id).await.unwrap();
    assert_eq!(expect_reason, reason);
}

#[when(regex = r"^(\S+) joins the room$")]
async fn when_member_joins_room(world: &mut World, id: String) {
    world.join_room(&id).await.unwrap();
}

#[when(regex = r"^(\S+)'s room closed by client$")]
async fn when_room_closed_by_client(world: &mut World, id: String) {
    world.close_room(&id).await.unwrap();
}

#[when(regex = r"^(\S+) disposes Jason object$")]
async fn when_jason_object_disposes(world: &mut World, id: String) {
    world.dispose_jason(&id).await.unwrap();
}

#[given(regex = r"^(\S+)'s `getUserMedia\(\)` (audio |video )?errors$")]
async fn given_member_gum_will_error(
    world: &mut World,
    id: String,
    kind: String,
) {
    let member = world.get_member(&id).unwrap();
    let media_devices = member.media_devices_mock();
    let (video, audio) = if kind.is_empty() {
        (true, true)
    } else {
        (kind.contains("video"), kind.contains("audio"))
    };
    media_devices.mock_gum(video, audio).await;
}

#[when(regex = "^(\\S+) enables (video|audio|video and audio) in local \
                 media settings$")]
async fn when_member_enables_via_local_media_settings(
    world: &mut World,
    id: String,
    kind: String,
) {
    let member = world.get_member(&id).unwrap();
    let video = kind.contains("video");
    let audio = kind.contains("audio");
    member
        .room()
        .set_local_media_settings(video, audio)
        .await
        .unwrap();
}

#[then(regex = "^(\\S+)'s `Room.on_failed_local_stream\\(\\)` fires (\\d+) \
                 time(:?s)?$")]
async fn then_room_failed_local_stream_fires(
    world: &mut World,
    id: String,
    times: u64,
) {
    let member = world.get_member(&id).unwrap();
    member.room().when_failed_local_stream_count(times).await;
}
