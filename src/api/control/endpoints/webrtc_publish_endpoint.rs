//! `WebRtcPublishEndpoint` [Control API]'s element implementation.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use derive_more::{Display, From, Into};
use serde::Deserialize;
use smart_default::SmartDefault;

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
    #[serde(default)]
    pub audio_settings: AudioSettings,

    /// Settings for the video media type of the [`WebRtcPublishEndpoint`].
    #[serde(default)]
    pub video_settings: VideoSettings,
}

/// Publishing policy of the video or audio media type in the
/// [`WebRtcPublishEndpoint`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, SmartDefault)]
pub enum PublishPolicy {
    /// Specified media type __may__ be published.
    ///
    /// Media server will try to initialize publishing, but won't produce any
    /// errors if user application will fail to or choose not to acquire
    /// required track. Media server will approve user request to stop and
    /// restart publishing specified media type.
    #[default]
    Optional,

    /// Specified media type __must__ be published.
    ///
    /// Media server will try to initialize publishing. If required media track
    /// could not be acquired, then an error will be thrown. Media server will
    /// deny all requests to stop publishing.
    Required,

    /// Media type __must__ not be published.
    ///
    /// Media server will not try to initialize publishing.
    Disabled,
}

impl PublishPolicy {
    /// Indicates whether publishing policy prescribes that media __should__ be
    /// published.
    #[inline]
    #[must_use]
    pub fn required(self) -> bool {
        match self {
            Self::Optional | Self::Disabled => false,
            Self::Required => true,
        }
    }
}

impl From<proto::web_rtc_publish_endpoint::PublishPolicy> for PublishPolicy {
    #[inline]
    fn from(from: proto::web_rtc_publish_endpoint::PublishPolicy) -> Self {
        use proto::web_rtc_publish_endpoint::PublishPolicy as Proto;
        match from {
            Proto::Optional => Self::Optional,
            Proto::Required => Self::Required,
            Proto::Disabled => Self::Disabled,
        }
    }
}

impl From<PublishPolicy> for proto::web_rtc_publish_endpoint::PublishPolicy {
    #[inline]
    fn from(from: PublishPolicy) -> Self {
        match from {
            PublishPolicy::Optional => Self::Optional,
            PublishPolicy::Required => Self::Required,
            PublishPolicy::Disabled => Self::Disabled,
        }
    }
}

/// Settings for the audio media type of the [`WebRtcPublishEndpoint`].
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct AudioSettings {
    /// Publishing policy of the audio media type in the
    /// [`WebRtcPublishEndpoint`].
    #[serde(default)]
    pub publish_policy: PublishPolicy,
}

impl From<&proto::web_rtc_publish_endpoint::AudioSettings> for AudioSettings {
    fn from(from: &proto::web_rtc_publish_endpoint::AudioSettings) -> Self {
        Self {
            publish_policy:
                proto::web_rtc_publish_endpoint::PublishPolicy::from_i32(
                    from.publish_policy,
                )
                .unwrap_or_default()
                .into(),
        }
    }
}

impl From<AudioSettings> for proto::web_rtc_publish_endpoint::AudioSettings {
    #[inline]
    fn from(from: AudioSettings) -> Self {
        use proto::web_rtc_publish_endpoint::PublishPolicy;
        Self {
            publish_policy: PublishPolicy::from(from.publish_policy).into(),
        }
    }
}

/// Settings for the video media type of the [`WebRtcPublishEndpoint`].
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct VideoSettings {
    /// Publishing policy of the video media type in the
    /// [`WebRtcPublishEndpoint`].
    #[serde(default)]
    pub publish_policy: PublishPolicy,
}

impl From<&proto::web_rtc_publish_endpoint::VideoSettings> for VideoSettings {
    fn from(from: &proto::web_rtc_publish_endpoint::VideoSettings) -> Self {
        Self {
            publish_policy:
                proto::web_rtc_publish_endpoint::PublishPolicy::from_i32(
                    from.publish_policy,
                )
                .unwrap_or_default()
                .into(),
        }
    }
}

impl From<VideoSettings> for proto::web_rtc_publish_endpoint::VideoSettings {
    #[inline]
    fn from(from: VideoSettings) -> Self {
        use proto::web_rtc_publish_endpoint::PublishPolicy;
        Self {
            publish_policy: PublishPolicy::from(from.publish_policy).into(),
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
                .map(AudioSettings::from)
                .unwrap_or_default(),
            video_settings: value
                .video_settings
                .as_ref()
                .map(VideoSettings::from)
                .unwrap_or_default(),
            force_relay: value.force_relay,
        }
    }
}
