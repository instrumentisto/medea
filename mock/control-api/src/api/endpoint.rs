//! `Endpoint` related methods and entities.

use medea_control_api_proto::grpc::api as proto;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// P2P mode of [`WebRtcPublishEndpoint`].
#[derive(Debug, Deserialize, Serialize)]
pub enum P2pMode {
    Always,
    Never,
    IfPossible,
}

impl From<P2pMode> for proto::web_rtc_publish_endpoint::P2p {
    fn from(mode: P2pMode) -> Self {
        match mode {
            P2pMode::Always => Self::Always,
            P2pMode::IfPossible => Self::IfPossible,
            P2pMode::Never => Self::Never,
        }
    }
}

impl From<proto::web_rtc_publish_endpoint::P2p> for P2pMode {
    fn from(proto: proto::web_rtc_publish_endpoint::P2p) -> Self {
        use proto::web_rtc_publish_endpoint::P2p;

        match proto {
            P2p::Always => Self::Always,
            P2p::IfPossible => Self::IfPossible,
            P2p::Never => Self::Never,
        }
    }
}

/// Publishing policy of the video or audio media type in the
/// [`WebRtcPublishEndpoint`].
#[derive(Clone, Copy, Debug, Deserialize, Serialize, SmartDefault)]
pub enum PublishPolicy {
    /// Publish this media type if it possible.
    #[default]
    Optional,

    /// Don't start call if this media type can't be published.
    Required,

    /// Media type __must__ not be published.
    ///
    /// Media server will not try to initialize publishing.
    Disabled,
}

impl From<proto::web_rtc_publish_endpoint::PublishPolicy> for PublishPolicy {
    fn from(proto: proto::web_rtc_publish_endpoint::PublishPolicy) -> Self {
        use proto::web_rtc_publish_endpoint::PublishPolicy::{
            Disabled, Optional, Required,
        };

        match proto {
            Optional => Self::Optional,
            Required => Self::Required,
            Disabled => Self::Disabled,
        }
    }
}

impl From<PublishPolicy> for proto::web_rtc_publish_endpoint::PublishPolicy {
    fn from(from: PublishPolicy) -> Self {
        match from {
            PublishPolicy::Optional => Self::Optional,
            PublishPolicy::Required => Self::Required,
            PublishPolicy::Disabled => Self::Disabled,
        }
    }
}

/// Settings for the audio media type of the [`WebRtcPublishEndpoint`].
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AudioSettings {
    /// Publishing policy of the audio media type in the
    /// [`WebRtcPublishEndpoint`].
    #[serde(default)]
    pub publish_policy: PublishPolicy,
}

impl From<proto::web_rtc_publish_endpoint::AudioSettings> for AudioSettings {
    fn from(proto: proto::web_rtc_publish_endpoint::AudioSettings) -> Self {
        Self {
            publish_policy:
                proto::web_rtc_publish_endpoint::PublishPolicy::from_i32(
                    proto.publish_policy,
                )
                .unwrap_or_default()
                .into(),
        }
    }
}

impl From<AudioSettings> for proto::web_rtc_publish_endpoint::AudioSettings {
    fn from(from: AudioSettings) -> Self {
        use proto::web_rtc_publish_endpoint::PublishPolicy;
        Self {
            publish_policy: PublishPolicy::from(from.publish_policy).into(),
        }
    }
}

/// Settings for the video media type of the [`WebRtcPublishEndpoint`].
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct VideoSettings {
    /// Publishing policy of the video media type in the
    /// [`WebRtcPublishEndpoint`].
    #[serde(default)]
    pub publish_policy: PublishPolicy,
}

impl From<VideoSettings> for proto::web_rtc_publish_endpoint::VideoSettings {
    fn from(from: VideoSettings) -> Self {
        use proto::web_rtc_publish_endpoint::PublishPolicy;
        Self {
            publish_policy: PublishPolicy::from(from.publish_policy).into(),
        }
    }
}

impl From<proto::web_rtc_publish_endpoint::VideoSettings> for VideoSettings {
    fn from(proto: proto::web_rtc_publish_endpoint::VideoSettings) -> Self {
        Self {
            publish_policy:
                proto::web_rtc_publish_endpoint::PublishPolicy::from_i32(
                    proto.publish_policy,
                )
                .unwrap_or_default()
                .into(),
        }
    }
}

/// [Control API]'s `WebRtcPublishEndpoint` representation.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Deserialize, Serialize)]
pub struct WebRtcPublishEndpoint {
    /// ID of [`WebRtcPublishEndpoint`].
    #[serde(skip_deserializing)]
    pub id: String,

    /// Mode of connection for this [`WebRtcPublishEndpoint`].
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

impl WebRtcPublishEndpoint {
    /// Converts [`WebRtcPublishEndpoint`] into protobuf
    /// [`proto::WebRtcPublishEndpoint`].
    #[must_use]
    pub fn into_proto(self, id: String) -> proto::WebRtcPublishEndpoint {
        let p2p: proto::web_rtc_publish_endpoint::P2p = self.p2p.into();
        proto::WebRtcPublishEndpoint {
            id,
            p2p: p2p as i32,
            force_relay: self.force_relay,
            on_start: String::new(),
            on_stop: String::new(),
            audio_settings: Some(self.audio_settings.into()),
            video_settings: Some(self.video_settings.into()),
        }
    }
}

impl From<proto::WebRtcPublishEndpoint> for WebRtcPublishEndpoint {
    fn from(proto: proto::WebRtcPublishEndpoint) -> Self {
        Self {
            id: proto.id,
            p2p: proto::web_rtc_publish_endpoint::P2p::from_i32(proto.p2p)
                .unwrap_or_default()
                .into(),
            force_relay: proto.force_relay,
            audio_settings: proto
                .audio_settings
                .map(Into::into)
                .unwrap_or_default(),
            video_settings: proto
                .video_settings
                .map(Into::into)
                .unwrap_or_default(),
        }
    }
}

/// [Control API]'s `WebRtcPlayEndpoint` element representation.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Deserialize, Serialize)]
pub struct WebRtcPlayEndpoint {
    /// ID of this [`WebRtcPlayEndpoint`].
    #[serde(skip_deserializing)]
    pub id: String,

    /// URI in format `local://{room_id}/{member_id}/{endpoint_id}` pointing to
    /// [`WebRtcPublishEndpoint`] which this [`WebRtcPlayEndpoint`] plays.
    pub src: String,

    /// Option to relay all media through a TURN server forcibly.
    #[serde(default)]
    pub force_relay: bool,
}

impl WebRtcPlayEndpoint {
    /// Converts [`WebRtcPlayEndpoint`] into protobuf
    /// [`proto::WebRtcPlayEndpoint`].
    #[must_use]
    pub fn into_proto(self, id: String) -> proto::WebRtcPlayEndpoint {
        proto::WebRtcPlayEndpoint {
            id,
            src: self.src,
            force_relay: self.force_relay,
            on_start: String::new(),
            on_stop: String::new(),
        }
    }
}

impl From<proto::WebRtcPlayEndpoint> for WebRtcPlayEndpoint {
    fn from(proto: proto::WebRtcPlayEndpoint) -> Self {
        Self {
            id: proto.id,
            src: proto.src,
            force_relay: proto.force_relay,
        }
    }
}

/// `Endpoint` element representation.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum Endpoint {
    WebRtcPublishEndpoint(WebRtcPublishEndpoint),
    WebRtcPlayEndpoint(WebRtcPlayEndpoint),
}

impl Endpoint {
    /// Converts [`Endpoint`] into protobuf [`proto::member::Element`].
    #[must_use]
    pub fn into_proto(self, id: String) -> proto::member::Element {
        let el = match self {
            Self::WebRtcPlayEndpoint(spec) => {
                proto::member::element::El::WebrtcPlay(spec.into_proto(id))
            }
            Self::WebRtcPublishEndpoint(spec) => {
                proto::member::element::El::WebrtcPub(spec.into_proto(id))
            }
        };
        proto::member::Element { el: Some(el) }
    }
}

impl From<proto::member::Element> for Endpoint {
    fn from(proto: proto::member::Element) -> Self {
        match proto.el.unwrap() {
            proto::member::element::El::WebrtcPub(webrtc_pub) => {
                Self::WebRtcPublishEndpoint(webrtc_pub.into())
            }
            proto::member::element::El::WebrtcPlay(webrtc_play) => {
                Self::WebRtcPlayEndpoint(webrtc_play.into())
            }
        }
    }
}
