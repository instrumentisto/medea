use cucumber_rust::when;

use crate::{parse_media_kind, world::World};

#[when(regex = "^Member `(.*)` (disables|mutes) (audio|video|all)$")]
async fn when_disables_mutes(
    world: &mut World,
    id: String,
    disable_or_mutes: String,
    audio_or_video: String,
) {
    let member = world.get_member(&id).unwrap();
    if disable_or_mutes == "disables" {
        member
            .toggle_media(parse_media_kind(&audio_or_video), None, false)
            .await
            .unwrap()
    } else {
        member
            .toggle_mute(parse_media_kind(&audio_or_video), None, true)
            .await
            .unwrap();
    }
}

#[when(regex = "^Member `(.*)` (enables|unmutes) (audio|video|all)$")]
async fn when_enables_mutes(
    world: &mut World,
    id: String,
    disable_or_mutes: String,
    audio_or_video: String,
) {
    let member = world.get_member(&id).unwrap();
    if disable_or_mutes == "enables" {
        member
            .toggle_media(parse_media_kind(&audio_or_video), None, true)
            .await
            .unwrap();
    } else {
        member
            .toggle_mute(parse_media_kind(&audio_or_video), None, false)
            .await
            .unwrap();
    }
}

#[when(regex = "^`(.*)` joins Room")]
async fn when_member_joins_room(world: &mut World, id: String) {
    world.join_room(&id).await.unwrap();
    world.wait_for_interconnection(&id).await.unwrap();
}

#[when(regex = "^`(.*)`'s Jason object disposes$")]
async fn when_jason_object_disposes(world: &mut World, id: String) {
    world.dispose_jason(&id).await.unwrap();
}

#[when(regex = "^Control API removes Member `(.*)`$")]
async fn when_control_api_removes_member(world: &mut World, id: String) {
    world.delete_member_element(&id).await;
}

#[when(regex = "^Control API removes Room$")]
async fn when_control_api_removes_room(world: &mut World) {
    world.delete_room_element().await;
}

#[when(
    regex = "Member `(.*)` enables remote (audio|(?:device |display )?video)"
)]
async fn when_member_enables_remote_track(
    world: &mut World,
    id: String,
    kind: String,
) {
    let member = world.get_member(&id).unwrap();
    let media_kind = kind.parse().unwrap();
    let source_kind = kind.parse().ok();
    member
        .room()
        .enable_remote_media(media_kind, source_kind)
        .await
        .unwrap();
}

#[when(regex = "^Member `(.*)` disables remote (audio|(?:device |display \
                )?video)$")]
async fn when_member_disables_remote_track(
    world: &mut World,
    id: String,
    kind: String,
) {
    let member = world.get_member(&id).unwrap();
    let media_kind = kind.parse().unwrap();
    let source_kind = kind.parse().ok();
    member
        .room()
        .disable_remote_media(media_kind, source_kind)
        .await
        .unwrap();
}

#[when(regex = "^`(.*)`'s Room closed by client$")]
async fn when_room_closed_by_client(world: &mut World, id: String) {
    world.close_room(&id).await.unwrap();
}
