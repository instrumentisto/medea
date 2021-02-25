use crate::world::{MembersPair, PairedMember, World};
use cucumber_rust::{then, when};
use medea_control_api_mock::proto::{AudioSettings, VideoSettings};
use std::time::Duration;
use tokio_1::time::timeout;

#[when(regex = "^Control API removes member (\\S*)$")]
async fn when_control_api_removes_member(world: &mut World, id: String) {
    world.delete_member_element(&id).await;
}

#[when(regex = "^Control API removes room$")]
async fn when_control_api_removes_room(world: &mut World) {
    world.delete_room_element().await;
}

#[when(
    regex = "^Control API interconnected (audio|video) of `(.*)` and `(.*)`$"
)]
async fn when_interconnects_kind(
    world: &mut World,
    kind: String,
    left_member_id: String,
    right_member_id: String,
) {
    let send_video = if kind.contains("video") {
        Some(VideoSettings {
            publish_policy: proto::PublishPolicy::Optional,
        })
    } else {
        None
    };
    use medea_control_api_mock::proto;
    let send_audio = if kind.contains("audio") {
        Some(AudioSettings {
            publish_policy: proto::PublishPolicy::Optional,
        })
    } else {
        None
    };

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

#[then(regex = "^Control API sends OnLeave callback with `(.*)` reason for \
                member (\\S*)$")]
async fn then_control_api_sends_on_leave(
    world: &mut World,
    reason: String,
    id: String,
) {
    timeout(Duration::from_secs(10), world.wait_for_on_leave(id, reason))
        .await
        .unwrap();
}

#[rustfmt::skip]
#[then(
regex = "^Control API doesn't sends OnLeave callback for member `(\\S*)`$"
)]
async fn then_control_api_doesnt_sends_on_leave(world: &mut World, id: String) {
    timeout(
        Duration::from_millis(300),
        world.wait_for_on_leave(id, "".to_string()),
    )
        .await
        .unwrap_err();
}

#[then(regex = "^Control API sends OnJoin callback for member (\\S*)$")]
async fn then_control_api_sends_on_join(world: &mut World, id: String) {
    timeout(Duration::from_secs(10), world.wait_for_on_join(id))
        .await
        .unwrap();
}
