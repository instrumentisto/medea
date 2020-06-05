//! `Endpoint` related methods and entities.

use medea_control_api_proto::grpc::api as proto;
use serde::{Deserialize, Serialize};

/// P2P mode of [`WebRtcPublishEndpoint`].
#[derive(Debug, Deserialize, Serialize)]
pub enum P2pMode {
    Always,
    Never,
    IfPossible,
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
#[derive(Debug, Deserialize, Serialize)]
pub enum PublishingPolicy {
    IfPossible,
    Required,
}

impl From<proto::web_rtc_publish_endpoint::PublishingPolicy>
    for PublishingPolicy
{
    fn from(proto: proto::web_rtc_publish_endpoint::PublishingPolicy) -> Self {
        use proto::web_rtc_publish_endpoint::PublishingPolicy::{
            PublishIfPossible, Required,
        };

        match proto {
            Required => Self::Required,
            PublishIfPossible => Self::IfPossible,
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
#[derive(Debug, Deserialize, Serialize)]
pub struct AudioSettings {
    /// Publishing policy of the audio media type in the
    /// [`WebRtcPublishEndpoint`].
    publishing_policy: PublishingPolicy,
}

impl From<proto::web_rtc_publish_endpoint::AudioSettings> for AudioSettings {
    fn from(proto: proto::web_rtc_publish_endpoint::AudioSettings) -> Self {
        Self {
            publishing_policy:
                proto::web_rtc_publish_endpoint::PublishingPolicy::from_i32(
                    proto.publishing_policy,
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
#[derive(Debug, Deserialize, Serialize)]
pub struct VideoSettings {
    /// Publishing policy of the video media type in the
    /// [`WebRtcPublishEndpoint`].
    publishing_policy: PublishingPolicy,
}

impl From<VideoSettings> for proto::web_rtc_publish_endpoint::VideoSettings {
    fn from(from: VideoSettings) -> Self {
        Self {
            publishing_policy: from.publishing_policy as i32,
        }
    }
}

impl From<proto::web_rtc_publish_endpoint::VideoSettings> for VideoSettings {
    fn from(proto: proto::web_rtc_publish_endpoint::VideoSettings) -> Self {
        Self {
            publishing_policy:
                proto::web_rtc_publish_endpoint::PublishingPolicy::from_i32(
                    proto.publishing_policy,
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
    id: String,

    /// Mode of connection for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// Option to relay all media through a TURN server forcibly.
    #[serde(default)]
    force_relay: bool,

    /// Settings for the audio media type of the [`WebRtcPublishEndpoint`].
    ///
    /// If `None` then audio shouldn't be published.
    audio_settings: Option<AudioSettings>,

    /// Settings for the video media type of the [`WebRtcPublishEndpoint`].
    ///
    /// If `None` then video shouldn't be published.
    video_settings: Option<VideoSettings>,
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
            audio_settings: self.audio_settings.map(Into::into),
            video_settings: self.video_settings.map(Into::into),
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
            audio_settings: proto.audio_settings.map(Into::into),
            video_settings: proto.video_settings.map(Into::into),
        }
    }
}

/// [Control API]'s `WebRtcPlayEndpoint` element representation.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Deserialize, Serialize)]
pub struct WebRtcPlayEndpoint {
    /// ID of `WebRtcPlayEndpoint`.
    #[serde(skip_deserializing)]
    id: String,

    /// URI in format `local://{room_id}/{member_id}/{endpoint_id}` pointing to
    /// [`WebRtcPublishEndpoint`] which this [`WebRtcPlayEndpoint`] plays.
    src: String,

    /// Option to relay all media through a TURN server forcibly.
    #[serde(default)]
    force_relay: bool,
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
