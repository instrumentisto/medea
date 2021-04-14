//! Platform-agnostic functionality of [`platform::RtcPeerConnection`].

use derive_more::{Display, From};

use crate::{
    platform::{self, RtcStatsError},
    utils::JsCaused,
};

/// Representation of [RTCSdpType].
///
/// [RTCSdpType]: https://w3.org/TR/webrtc/#dom-rtcsdptype
pub enum SdpType {
    /// [`offer` type][1] of SDP.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcsdptype-offer
    Offer(String),

    /// [`answer` type][1] of SDP.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcsdptype-answer
    Answer(String),
}

pub enum RtcSdpType {
    Offer,
    Pranswer,
    Answer,
    Rollback,
}

impl From<RtcSdpType> for web_sys::RtcSdpType {
    fn from(from: RtcSdpType) -> Self {
        match from {
            RtcSdpType::Offer => Self::Offer,
            RtcSdpType::Pranswer => Self::Pranswer,
            RtcSdpType::Answer => Self::Answer,
            RtcSdpType::Rollback => Self::Rollback,
        }
    }
}

/// [RTCIceCandidate][1] representation.
///
/// [1]: https://w3.org/TR/webrtc/#rtcicecandidate-interface
pub struct IceCandidate {
    /// [`candidate` field][2] of the discovered [RTCIceCandidate][1].
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcicecandidate
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcicecandidate-candidate
    pub candidate: String,

    /// [`sdpMLineIndex` field][2] of the discovered [RTCIceCandidate][1].
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcicecandidate
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcicecandidate-sdpmlineindex
    pub sdp_m_line_index: Option<u16>,

    /// [`sdpMid` field][2] of the discovered [RTCIceCandidate][1].
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcicecandidate
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcicecandidate-sdpmid
    pub sdp_mid: Option<String>,
}

/// Errors that may occur during signaling between this and remote
/// [RTCPeerConnection][1] and event handlers setting errors.
///
/// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
#[derive(Clone, Debug, Display, From, JsCaused)]
#[js(error = "platform::Error")]
pub enum RtcPeerConnectionError {
    /// Occurs when cannot adds new remote candidate to the
    /// [RTCPeerConnection][1]'s remote description.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    #[display(fmt = "Failed to add ICE candidate: {}", _0)]
    #[from(ignore)]
    AddIceCandidateFailed(platform::Error),

    /// Occurs when cannot obtains [SDP answer][`SdpType::Answer`] from
    /// the underlying [`platform::RtcPeerConnection`].
    #[display(fmt = "Failed to create SDP answer: {}", _0)]
    #[from(ignore)]
    CreateAnswerFailed(platform::Error),

    /// Occurs when a new [`platform::RtcPeerConnection`] cannot be created.
    #[display(fmt = "Failed to create PeerConnection: {}", _0)]
    #[from(ignore)]
    PeerCreationError(platform::Error),

    /// Occurs when cannot obtains [SDP offer][`SdpType::Offer`] from
    /// the underlying [`platform::RtcPeerConnection`].
    #[display(fmt = "Failed to create SDP offer: {}", _0)]
    #[from(ignore)]
    CreateOfferFailed(platform::Error),

    /// Occurs while getting and parsing [`platform::RtcStats`] of
    /// [`platform::RtcPeerConnection`].
    #[display(fmt = "Failed to get RTCStats: {}", _0)]
    RtcStatsError(#[js(cause)] RtcStatsError),

    /// [PeerConnection.getStats][1] promise thrown exception.
    ///
    /// [1]: https://tinyurl.com/w6hmt5f
    #[display(fmt = "PeerConnection.getStats() failed with error: {}", _0)]
    #[from(ignore)]
    GetStatsException(platform::Error),

    /// Occurs if the local description associated with the
    /// [`platform::RtcPeerConnection`] cannot be changed.
    #[display(fmt = "Failed to set local SDP description: {}", _0)]
    #[from(ignore)]
    SetLocalDescriptionFailed(platform::Error),

    /// Occurs if the description of the remote end of the
    /// [`platform::RtcPeerConnection`] cannot be changed.
    #[display(fmt = "Failed to set remote SDP description: {}", _0)]
    #[from(ignore)]
    SetRemoteDescriptionFailed(platform::Error),
}
