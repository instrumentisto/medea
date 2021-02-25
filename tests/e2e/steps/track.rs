use cucumber_rust::then;

use crate::{
    object::{MediaKind, MediaSourceKind},
    parse_media_kinds,
    world::World,
};

#[then(regex = "^Member (\\S*) has (\\d*) local tracks$")]
async fn then_member_has_local_tracks(
    world: &mut World,
    id: String,
    count: u64,
) {
    let member = world.get_member(&id).unwrap();
    let room = member.room();
    let tracks = room.local_tracks().await.unwrap();
    assert_eq!(count, tracks.count().await.unwrap());
}

#[then(
    regex = "^(\\S*) has (audio|video|audio and video) remote track(?:s)? \
             with (\\S*)"
)]
async fn then_member_has_remote_track(
    world: &mut World,
    id: String,
    kind: String,
    remote_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let connection = member
        .connections()
        .wait_for_connection(remote_id)
        .await
        .unwrap();
    let tracks_store = connection.tracks_store().await.unwrap();

    if kind.contains("audio") {
        tracks_store
            .get_track(MediaKind::Audio, MediaSourceKind::Device)
            .await
            .unwrap();
    }
    if kind.contains("video") {
        tracks_store
            .get_track(MediaKind::Video, MediaSourceKind::Device)
            .await
            .unwrap();
    }
}

// #[then(regex = "^(\\S*) has local (audio|(?:device |display )?video)$")]
// async fn then_has_local_track(world: &mut World, id: String, kind: String) {
//     let member = world.get_member(&id).unwrap();
//     let room = member.room();
//     let tracks = room.local_tracks().await.unwrap();
//     let media_kind = kind.parse().unwrap();
//     let source_kind = kind.parse().ok();
//
//     assert!(tracks.has_track(media_kind, source_kind).await.unwrap())
// }
#[then(regex = "^(\\S*) has local (audio|(?:device |display )?video)$")]
async fn then_has_local_track(world: &mut World, id: String, kind: String) {
    let member = world.get_member(&id).unwrap();
    let room = member.room();
    let tracks = room.local_tracks().await.unwrap();
    let media_kind = kind.parse().unwrap();
    // FIXME
    // let source_kind = kind.parse().ok();

    tracks
        .get_track(media_kind, MediaSourceKind::Device)
        .await
        .unwrap();
}

#[then(regex = "^(\\S*) remote (audio|(?:device|display) video) track from \
                (\\S*) disables")]
async fn then_remote_track_stops(
    world: &mut World,
    id: String,
    kind: String,
    remote_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();

    let conn = member.connections().get(remote_id).await.unwrap().unwrap();
    let track = conn
        .tracks_store()
        .await
        .unwrap()
        .get_track(media_kind, source_kind)
        .await
        .unwrap();
    assert!(track.muted().await.unwrap());
}

#[then(regex = "^on_disabled callback fires (\\d*) time on (\\S*)'s remote \
                (audio|(?:device|display) video) track from (\\S*)$")]
async fn then_on_remote_disabled_callback_fires(
    world: &mut World,
    times: u64,
    id: String,
    kind: String,
    remote_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let remote_conn =
        member.connections().get(remote_id).await.unwrap().unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();

    let track = remote_conn
        .tracks_store()
        .await
        .unwrap()
        .get_track(media_kind, source_kind)
        .await
        .unwrap();
    track.wait_for_on_disabled_fire_count(times).await.unwrap();
}

#[then(regex = "^on_enabled callback fires (\\d*) time on (\\S*)'s remote \
                (audio|(?:device|display) video) track from (\\S*)$")]
async fn then_on_remote_enabled_callback_fires(
    world: &mut World,
    times: u64,
    id: String,
    kind: String,
    remote_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let remote_conn =
        member.connections().get(remote_id).await.unwrap().unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();
    let track = remote_conn
        .tracks_store()
        .await
        .unwrap()
        .get_track(media_kind, source_kind)
        .await
        .unwrap();
    track.wait_for_on_enabled_fire_count(times).await.unwrap();
}

#[then(
    regex = "^(\\S*)'s (audio|(?:display|device) video) remote track with \
             (\\S*) is (enabled|disabled)$"
)]
async fn then_remote_media_track(
    world: &mut World,
    id: String,
    kind: String,
    partner_id: String,
    state: String,
) {
    let member = world.get_member(&id).unwrap();
    let partner_connection = member
        .connections()
        .wait_for_connection(partner_id)
        .await
        .unwrap();
    let tracks_with_partner = partner_connection.tracks_store().await.unwrap();

    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();
    let track = tracks_with_partner
        .get_track(media_kind, source_kind)
        .await
        .unwrap();

    match state.as_str() {
        "enabled" => track.wait_for_enabled().await.unwrap(),
        "disabled" => track.wait_for_disabled().await.unwrap(),
        _ => unreachable!(),
    };
}

#[then(
    regex = "^(\\S*) doesn't have (audio|(?:device|display) video) remote \
             track with (\\S*)$"
)]
async fn then_doesnt_have_remote_track(
    world: &mut World,
    id: String,
    kind: String,
    partner_id: String,
) {
    let member = world.get_member(&id).unwrap();
    let partner_connection = member
        .connections()
        .wait_for_connection(partner_id)
        .await
        .unwrap();
    let tracks_with_partner = partner_connection.tracks_store().await.unwrap();
    let (media_kind, source_kind) = parse_media_kinds(&kind).unwrap();

    assert!(!tracks_with_partner
        .has_track(media_kind, Some(source_kind))
        .await
        .unwrap());
}
