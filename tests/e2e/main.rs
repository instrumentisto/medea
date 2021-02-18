#![allow(clippy::module_name_repetitions)]

mod browser;
mod conf;
mod control;
mod object;
mod steps;
mod world;

use std::str::FromStr;

use cucumber_rust::{given, when, WorldInit as _};
use tokio_1 as tokio;

use self::{
    object::MediaKind,
    world::{MemberBuilder, World},
};

#[tokio::main]
async fn main() {
    let runner = World::init(&[conf::FEATURES_PATH.as_str()]);
    runner.run_and_exit().await;
}

fn parse_media_kind(text: &str) -> Option<MediaKind> {
    match text {
        "audio" => Some(MediaKind::Audio),
        "video" => Some(MediaKind::Video),
        "all" => None,
        _ => unreachable!(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Matched(pub bool);

impl FromStr for Matched {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(!s.is_empty()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MediaSettings {
    DisabledMedia,
    MutedMedia,
    NoWebRtcEndpoint,
    None,
}

impl FromStr for MediaSettings {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("no WebRTC endpoints") {
            Ok(Self::NoWebRtcEndpoint)
        } else if s.contains("disabled") {
            Ok(Self::DisabledMedia)
        } else if s.contains("muted") {
            Ok(Self::MutedMedia)
        } else {
            Ok(Self::None)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DisabledMediaType {
    Audio,
    Video,
    All,
    None,
}

impl FromStr for DisabledMediaType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("audio") {
            Ok(Self::Audio)
        } else if s.contains("video") {
            Ok(Self::Video)
        } else if s.contains("media") {
            Ok(Self::All)
        } else {
            Ok(Self::None)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Publish,
    Play,
    None,
}

impl FromStr for Direction {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("publishing") {
            Ok(Self::Publish)
        } else if s.contains("playing") {
            Ok(Self::Play)
        } else {
            Ok(Self::None)
        }
    }
}

#[given(regex = "^(?:room with )?(joined )?member(?:s)? (\\S*)(?:(?:, | and \
                 )(\\S*)(?: and (\\S*)?)?)?(?: with (no (play |publish \
                 )?WebRTC endpoints|(?:disabled|muted) (media|audio|video) \
                 (publishing|playing)?))?$")]
#[async_recursion::async_recursion]
async fn new_given_member(
    world: &mut World,
    joined: Matched,
    first_member_id: String,
    second_member_id: String,
    third_member_id: String,
    media_settings: MediaSettings,
    not_endpoint_direction: Direction,
    disabled_media_type: DisabledMediaType,
    disabled_direction: Direction,
) {
    let endpoints_disabled = media_settings == MediaSettings::NoWebRtcEndpoint;
    let all_endpoints_disabled =
        endpoints_disabled && not_endpoint_direction == Direction::None;
    let is_send_disabled = endpoints_disabled
        && (all_endpoints_disabled
            || not_endpoint_direction == Direction::Publish);
    let is_recv_disabled = endpoints_disabled
        && (all_endpoints_disabled
            || not_endpoint_direction == Direction::Play);

    let member_builder = MemberBuilder {
        id: first_member_id.clone(),
        is_send: !is_send_disabled,
        is_recv: !is_recv_disabled,
    };
    world.create_member(member_builder).await.unwrap();
    if joined.0 {
        world.join_room(&first_member_id).await.unwrap();
        world
            .wait_for_interconnection(&first_member_id)
            .await
            .unwrap();
    }

    let member = world.get_member(&first_member_id).unwrap();
    match media_settings {
        MediaSettings::DisabledMedia => {
            let is_audio = !(disabled_media_type != DisabledMediaType::Audio);
            let is_video = !(disabled_media_type != DisabledMediaType::Video);
            let is_publish = !(disabled_direction != Direction::Publish);
            let is_play = !(disabled_direction != Direction::Play);

            if is_publish {
                if is_audio {
                    member
                        .disable_media_send(MediaKind::Audio, None)
                        .await
                        .unwrap();
                }
                if is_video {
                    member
                        .disable_media_send(MediaKind::Video, None)
                        .await
                        .unwrap();
                }
            }
            if is_play {
                todo!("Play disabling is not implemented atm");
            }
        }
        MediaSettings::MutedMedia => {
            todo!("Muting is not implemented atm");
        }
        _ => (),
    }

    if !second_member_id.is_empty() {
        new_given_member(
            world,
            joined,
            second_member_id,
            third_member_id,
            String::new(),
            media_settings,
            not_endpoint_direction,
            disabled_media_type,
            disabled_direction,
        )
        .await;
    }
}

#[when(regex = "^(\\S*) joins room")]
async fn when_member_joins_room(world: &mut World, id: String) {
    world.join_room(&id).await.unwrap();
}
