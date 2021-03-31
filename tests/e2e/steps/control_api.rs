use std::time::Duration;

use cucumber_rust::{then, when};
use medea_control_api_mock::proto::{
    self as proto, AudioSettings, VideoSettings,
};
use tokio_1::time::timeout;

use crate::world::{MembersPair, PairedMember, World};

#[when(regex = r"^Control API removes member (\S+)$")]
async fn when_control_api_removes_member(world: &mut World, id: String) {
    world.delete_member_element(&id).await;
}

#[when(regex = r"^Control API removes the room$")]
async fn when_control_api_removes_room(world: &mut World) {
    world.delete_room_element().await;
}

#[when(regex = r"^Control API interconnects (audio|video) of (\S+) and (\S+)$")]
async fn when_interconnects_kind(
    world: &mut World,
    kind: String,
    left_member_id: String,
    right_member_id: String,
) {
    let send_video = kind.contains("video").then(|| VideoSettings {
        publish_policy: proto::PublishPolicy::Optional,
    });
    let send_audio = kind.contains("audio").then(|| AudioSettings {
        publish_policy: proto::PublishPolicy::Optional,
    });

    world
        .interconnect_members(MembersPair {
            left: PairedMember {
                id: left_member_id,
                recv: true,
                send_video: send_video.clone(),
                send_audio: send_audio.clone(),
            },
            right: PairedMember {
                id: right_member_id,
                recv: true,
                send_video,
                send_audio,
            },
        })
        .await
        .unwrap();
}

#[then(regex = "^Control API sends `OnLeave` callback with `(.+)` reason \
                 for member (\\S+)$")]
async fn then_control_api_sends_on_leave(
    world: &mut World,
    reason: String,
    id: String,
) {
    // Assertion is done inside `wait_for_on_leave()` method.
    timeout(Duration::from_secs(10), world.wait_for_on_leave(id, reason))
        .await
        .unwrap();
}

#[then(regex = "^Control API doesn't send `OnLeave` callback for \
                 member (\\S+)$")]
async fn then_control_api_doesnt_sends_on_leave(world: &mut World, id: String) {
    assert!(timeout(
        Duration::from_millis(300),
        world.wait_for_on_leave(id, String::new()),
    )
    .await
    .is_err());
}

#[then(regex = r"^Control API sends `OnJoin` callback for member (\S+)$")]
async fn then_control_api_sends_on_join(world: &mut World, id: String) {
    assert!(timeout(Duration::from_secs(10), world.wait_for_on_join(id))
        .await
        .is_ok());
}

#[when(regex = "^Control API starts (\\S+)'s (audio|video|media) publishing \
                 to (\\S+)$")]
async fn when_control_api_starts_publishing(
    world: &mut World,
    publisher_id: String,
    kind: String,
    receiver_id: String,
) {
    let all_kinds = kind.contains("media");
    let send_audio =
        (all_kinds || kind.contains("audio")).then(AudioSettings::default);
    let send_video =
        (all_kinds || kind.contains("video")).then(VideoSettings::default);
    world
        .interconnect_members(MembersPair {
            left: PairedMember {
                id: publisher_id,
                recv: false,
                send_audio,
                send_video,
            },
            right: PairedMember {
                id: receiver_id,
                recv: true,
                send_video: None,
                send_audio: None,
            },
        })
        .await
        .unwrap();
}

#[when(regex = r"^Control API interconnects (\S+) and (\S+)$")]
async fn when_control_api_interconnects_members(
    world: &mut World,
    id: String,
    partner_id: String,
) {
    world
        .interconnect_members(MembersPair {
            left: PairedMember {
                id,
                recv: true,
                send_video: Some(VideoSettings::default()),
                send_audio: Some(AudioSettings::default()),
            },
            right: PairedMember {
                id: partner_id,
                recv: true,
                send_video: Some(VideoSettings::default()),
                send_audio: Some(AudioSettings::default()),
            },
        })
        .await
        .unwrap();
}

#[when(regex = r"^Control API removes (\S+) by apply$")]
async fn when_control_api_removes_member_by_apply(
    world: &mut World,
    id: String,
) {
    let mut spec = world.get_spec().await;
    spec.pipeline.remove(&id).unwrap();
    world.apply(spec).await;
}

#[when(regex = r"Control API interconnects (\S+) and (\S+) by apply")]
async fn when_control_api_interconnects_by_apply(
    world: &mut World,
    id: String,
    partner_id: String,
) {
    world
        .interconnect_members_by_apply(MembersPair {
            left: PairedMember {
                id,
                recv: true,
                send_video: Some(VideoSettings::default()),
                send_audio: Some(AudioSettings::default()),
            },
            right: PairedMember {
                id: partner_id,
                recv: true,
                send_video: Some(VideoSettings::default()),
                send_audio: Some(AudioSettings::default()),
            },
        })
        .await;
}

#[when(regex = r"^Control API deletes (\S+)'s publish endpoint$")]
async fn when_control_api_deletes_publish_endpoint(
    world: &mut World,
    id: String,
) {
    world.delete_publish_endpoint(&id).await;
    tokio_1::time::sleep(std::time::Duration::from_millis(200)).await;
}

#[when(regex = r"^Control API deletes (\S+)'s play endpoint with (\S+)$")]
async fn when_control_api_deletes_play_endpoint(
    world: &mut World,
    id: String,
    partner_id: String,
) {
    world.delete_play_endpoint(&id, &partner_id).await;
    tokio_1::time::sleep(std::time::Duration::from_millis(200)).await;
}
