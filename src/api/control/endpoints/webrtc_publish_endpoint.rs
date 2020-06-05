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

    /// Settings for the audio media type of the [`WebRtcPublishEndpoint`].
    ///
    /// If `None` then audio shouldn't be published.
    pub audio_settings: Option<AudioSettings>,

    /// Settings for the video media type of the [`WebRtcPublishEndpoint`].
    ///
    /// If `None` then video shouldn't be published.
    pub video_settings: Option<VideoSettings>,
}

/// Publishing policy of the video or audio media type in the
/// [`WebRtcPublishEndpoint`].
#[derive(Clone, Copy, Debug, Deserialize)]
pub enum PublishingPolicy {
    /// Specified media type __may__ be published.
    ///
    /// Media server will try to initialize publishing, but won't produce any
    /// errors if user application will fail to or choose not to acquire
    /// required track. Media server will approve user request to stop and
    /// restart publishing specified media type.
    IfPossible,

    /// Specified media type __must__ be published.
    ///
    /// Media server will try to initialize publishing. If required media track
    /// could not be acquired, then an error will be thrown. Media server will
    /// deny all requests to stop publishing.
    Required,
}

impl PublishingPolicy {
    /// Returns `true` if publishing policy prescribes that media __should__ be
    /// published.
    ///
    /// If `false` then media can be not published.
    pub fn is_required(self) -> bool {
        match self {
            PublishingPolicy::IfPossible => false,
            PublishingPolicy::Required => true,
        }
    }
}

impl From<proto::web_rtc_publish_endpoint::PublishingPolicy>
    for PublishingPolicy
{
    fn from(from: proto::web_rtc_publish_endpoint::PublishingPolicy) -> Self {
        use proto::web_rtc_publish_endpoint::PublishingPolicy::{
            PublishIfPossible, Required,
        };

        match from {
            PublishIfPossible => Self::IfPossible,
            Required => Self::Required,
        }
    }
}

impl From<PublishingPolicy>
    for proto::web_rtc_publish_endpoint::PublishingPolicy
{
    fn from(from: PublishingPolicy) -> Self {
        match from {
            PublishingPolicy::IfPossible => Self::PublishIfPossible,
            PublishingPolicy::Required => Self::Required,
        }
    }
}

/// Settings for the audio media type of the [`WebRtcPublishEndpoint`].
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct AudioSettings {
    /// Publishing policy of the audio media type in the
    /// [`WebRtcPublishEndpoint`].
    pub publishing_policy: PublishingPolicy,
}

impl From<&proto::web_rtc_publish_endpoint::AudioSettings> for AudioSettings {
    fn from(from: &proto::web_rtc_publish_endpoint::AudioSettings) -> Self {
        Self {
            publishing_policy:
                proto::web_rtc_publish_endpoint::PublishingPolicy::from_i32(
                    from.publishing_policy,
                )
                .unwrap_or_default()
                .into(),
        }
    }
}

impl From<AudioSettings> for proto::web_rtc_publish_endpoint::AudioSettings {
    fn from(from: AudioSettings) -> Self {
        Self {
            publishing_policy: from.publishing_policy as i32,
        }
    }
}

/// Settings for the video media type of the [`WebRtcPublishEndpoint`].
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct VideoSettings {
    /// Publishing policy of the video media type in the
    /// [`WebRtcPublishEndpoint`].
    pub publishing_policy: PublishingPolicy,
}

impl From<&proto::web_rtc_publish_endpoint::VideoSettings> for VideoSettings {
    fn from(from: &proto::web_rtc_publish_endpoint::VideoSettings) -> Self {
        Self {
            publishing_policy:
                proto::web_rtc_publish_endpoint::PublishingPolicy::from_i32(
                    from.publishing_policy,
                )
                .unwrap_or_default()
                .into(),
        }
    }
}

impl From<VideoSettings> for proto::web_rtc_publish_endpoint::VideoSettings {
    fn from(from: VideoSettings) -> Self {
        Self {
            publishing_policy: from.publishing_policy as i32,
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
                .map(AudioSettings::from),
            video_settings: value
                .video_settings
                .as_ref()
                .map(VideoSettings::from),
            force_relay: value.force_relay,
        }
    }
}
