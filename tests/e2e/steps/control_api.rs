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

#[when(
    regex = r"^Control API interconnected (audio|video) of (\S+) and (\S+)$"
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

#[then(regex = "^Control API sends OnLeave callback with `(.+)` reason for \
                member (\\S+)$")]
async fn then_control_api_sends_on_leave(
    world: &mut World,
    reason: String,
    id: String,
) {
    timeout(Duration::from_secs(10), world.wait_for_on_leave(id, reason))
        .await
        .unwrap();
}

#[then(
    regex = r"^Control API doesn't sends `OnLeave` callback for member (\S+)$"
)]
async fn then_control_api_doesnt_sends_on_leave(world: &mut World, id: String) {
    timeout(
        Duration::from_millis(300),
        world.wait_for_on_leave(id, "".to_string()),
    )
    .await
    .unwrap_err();
}

#[then(regex = r"^Control API sends `OnJoin` callback for member (\S+)$")]
async fn then_control_api_sends_on_join(world: &mut World, id: String) {
    timeout(Duration::from_secs(10), world.wait_for_on_join(id))
        .await
        .unwrap();
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
    let send_audio = if all_kinds || kind.contains("audio") {
        Some(AudioSettings::default())
    } else {
        None
    };
    let send_video = if all_kinds || kind.contains("video") {
        Some(VideoSettings::default())
    } else {
        None
    };
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
