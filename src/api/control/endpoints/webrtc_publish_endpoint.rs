//! `WebRtcPublishEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::convert::From;

use derive_more::{Display, From, Into};
use serde::Deserialize;

use medea_control_api_proto::grpc::api as proto;

/// ID of [`WebRtcPublishEndpoint`].
#[derive(
    Clone, Debug, Deserialize, Display, Eq, Hash, PartialEq, From, Into,
)]
pub struct WebRtcPublishId(String);

/// Peer-to-peer mode of [`WebRtcPublishEndpoint`].
#[derive(Clone, Copy, Deserialize, Debug)]
pub enum P2pMode {
    /// Always connect peer-to-peer.
    Always,

    /// Never connect peer-to-peer.
    Never,

    /// Connect peer-to-peer if it possible.
    IfPossible,
}

impl From<proto::web_rtc_publish_endpoint::P2p> for P2pMode {
    fn from(value: proto::web_rtc_publish_endpoint::P2p) -> Self {
        use proto::web_rtc_publish_endpoint::P2p;

        match value {
            P2p::Always => Self::Always,
            P2p::IfPossible => Self::IfPossible,
            P2p::Never => Self::Never,
        }
    }
}

impl Into<proto::web_rtc_publish_endpoint::P2p> for P2pMode {
    fn into(self) -> proto::web_rtc_publish_endpoint::P2p {
        use proto::web_rtc_publish_endpoint::P2p;

        match self {
            Self::Always => P2p::Always,
            Self::IfPossible => P2p::IfPossible,
            Self::Never => P2p::Never,
        }
    }
}

/// Media element which is able to publish media data for another client via
/// WebRTC.
#[derive(Clone, Deserialize, Debug)]
pub struct WebRtcPublishEndpoint {
    /// Peer-to-peer mode of this [`WebRtcPublishEndpoint`].
    pub p2p: P2pMode,

    /// Option to relay all media through a TURN server forcibly.
    #[serde(default)]
    pub force_relay: bool,

    pub audio_settings: Option<AudioSettings>,

    pub video_settings: Option<VideoSettings>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub enum PublishingMode {
    IfPossible,
    Required,
}

impl PublishingMode {
    pub fn is_important(&self) -> bool {
        match self {
            PublishingMode::IfPossible => false,
            PublishingMode::Required => true,
        }
    }
}

impl From<proto::web_rtc_publish_endpoint::PublishingMode> for PublishingMode {
    fn from(from: proto::web_rtc_publish_endpoint::PublishingMode) -> Self {
        use proto::web_rtc_publish_endpoint::PublishingMode as PM;

        match from {
            PM::IfPossible => Self::IfPossible,
            PM::Required => Self::Required,
        }
    }
}

impl From<PublishingMode> for proto::web_rtc_publish_endpoint::PublishingMode {
    fn from(from: PublishingMode) -> Self {
        match from {
            PublishingMode::IfPossible => Self::IfPossible,
            PublishingMode::Required => Self::Required,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct AudioSettings {
    pub publishing_mode: PublishingMode,
}

impl From<&proto::web_rtc_publish_endpoint::AudioSettings> for AudioSettings {
    fn from(from: &proto::web_rtc_publish_endpoint::AudioSettings) -> Self {
        Self {
            publishing_mode:
                proto::web_rtc_publish_endpoint::PublishingMode::from_i32(
                    from.publishing_mode,
                )
                .unwrap_or_default()
                .into(),
        }
    }
}

impl From<AudioSettings> for proto::web_rtc_publish_endpoint::AudioSettings {
    fn from(from: AudioSettings) -> Self {
        Self {
            publishing_mode:
                proto::web_rtc_publish_endpoint::PublishingMode::from(
                    from.publishing_mode,
                ) as i32,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct VideoSettings {
    pub publishing_mode: PublishingMode,
}

impl From<&proto::web_rtc_publish_endpoint::VideoSettings> for VideoSettings {
    fn from(from: &proto::web_rtc_publish_endpoint::VideoSettings) -> Self {
        Self {
            publishing_mode:
                proto::web_rtc_publish_endpoint::PublishingMode::from_i32(
                    from.publishing_mode,
                )
                .unwrap_or_default()
                .into(),
        }
    }
}

impl From<VideoSettings> for proto::web_rtc_publish_endpoint::VideoSettings {
    fn from(from: VideoSettings) -> Self {
        Self {
            publishing_mode:
                proto::web_rtc_publish_endpoint::PublishingMode::from(
                    from.publishing_mode,
                ) as i32,
        }
    }
}

impl From<&proto::WebRtcPublishEndpoint> for WebRtcPublishEndpoint {
    fn from(value: &proto::WebRtcPublishEndpoint) -> Self {
        Self {
            p2p: P2pMode::from(
                proto::web_rtc_publish_endpoint::P2p::from_i32(value.p2p)
                    .unwrap_or_default(),
            ),
            audio_settings: value
                .audio_settings
                .as_ref()
                .map(|s| AudioSettings::from(s)),
            video_settings: value
                .video_settings
                .as_ref()
                .map(|s| VideoSettings::from(s)),
            force_relay: value.force_relay,
        }
    }
}
