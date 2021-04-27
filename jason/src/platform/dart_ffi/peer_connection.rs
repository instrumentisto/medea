//! Wrapper around [RTCPeerConnection][1].
//!
//! [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection

use medea_client_api_proto::{
    IceConnectionState, IceServer, PeerConnectionState,
};
use tracerr::Traced;

use crate::{
    media::{MediaKind, TrackConstraints},
    platform::{
        IceCandidate, MediaStreamTrack, RtcPeerConnectionError, RtcStats,
        SdpType, Transceiver, TransceiverDirection,
    },
};

impl From<&TrackConstraints> for MediaKind {
    fn from(media_type: &TrackConstraints) -> Self {
        match media_type {
            TrackConstraints::Audio(_) => Self::Audio,
            TrackConstraints::Video(_) => Self::Video,
        }
    }
}

type Result<T> = std::result::Result<T, Traced<RtcPeerConnectionError>>;

/// Representation of [RTCPeerConnection][1].
///
/// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
pub struct RtcPeerConnection {}

impl RtcPeerConnection {
    /// Instantiates new [`RtcPeerConnection`].
    ///
    /// # Errors
    ///
    /// Errors with [`RtcPeerConnectionError::PeerCreationError`] if
    /// [`SysRtcPeerConnection`] creation fails.
    pub fn new<I>(ice_servers: I, is_force_relayed: bool) -> Result<Self>
    where
        I: IntoIterator<Item = IceServer>,
    {
        unimplemented!()
    }

    /// Returns [`RtcStats`] of this [`RtcPeerConnection`].
    ///
    /// # Errors
    ///
    /// Errors with [`RtcPeerConnectionError::RtcStatsError`] if getting or
    /// parsing of [`RtcStats`] fails.
    ///
    /// Errors with [`RtcPeerConnectionError::GetStatsException`] when
    /// [PeerConnection.getStats][1] promise throws exception.
    ///
    /// [1]: https://tinyurl.com/w6hmt5f
    pub async fn get_stats(&self) -> Result<RtcStats> {
        unimplemented!()
    }

    /// Sets handler for a [`RtcTrackEvent`] (see [RTCTrackEvent][1] and
    /// [`ontrack` callback][2]).
    ///
    /// # Panics
    ///
    /// If binding to the [`track`][3] event fails. Not supposed to ever happen.
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtctrackevent
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection-ontrack
    /// [3]: https://w3.org/TR/webrtc/#event-track
    pub fn on_track<F>(&self, f: Option<F>)
    where
        F: 'static + FnMut(MediaStreamTrack, Transceiver),
    {
        unimplemented!()
    }

    /// Sets handler for a [`RtcPeerConnectionIceEvent`] (see
    /// [RTCPeerConnectionIceEvent][1] and [`onicecandidate` callback][2]).
    ///
    /// # Panics
    ///
    /// If binding to the [`icecandidate`][3] event fails. Not supposed to ever
    /// happen.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnectioniceevent
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection-onicecandidate
    /// [3]: https://w3.org/TR/webrtc/#event-icecandidate
    pub fn on_ice_candidate<F>(&self, f: Option<F>)
    where
        F: 'static + FnMut(IceCandidate),
    {
        unimplemented!()
    }

    /// Returns [`RtcIceConnectionState`] of this [`RtcPeerConnection`].
    #[inline]
    #[must_use]
    pub fn ice_connection_state(&self) -> IceConnectionState {
        unimplemented!()
    }

    /// Returns [`PeerConnectionState`] of this [`RtcPeerConnection`].
    ///
    /// Returns [`None`] if failed to parse a [`PeerConnectionState`].
    #[inline]
    #[must_use]
    pub fn connection_state(&self) -> Option<PeerConnectionState> {
        unimplemented!()
    }

    /// Sets handler for an [`iceconnectionstatechange`][1] event.
    ///
    /// # Panics
    ///
    /// If binding to the [`iceconnectionstatechange`][1] event fails. Not
    /// supposed to ever happen.
    ///
    /// [1]: https://w3.org/TR/webrtc/#event-iceconnectionstatechange
    pub fn on_ice_connection_state_change<F>(&self, f: Option<F>)
    where
        F: 'static + FnMut(IceConnectionState),
    {
        unimplemented!()
    }

    /// Sets handler for a [`connectionstatechange`][1] event.
    ///
    /// # Panics
    ///
    /// If binding to the [`connectionstatechange`][1] event fails. Not supposed
    /// to ever happen.
    ///
    /// [1]: https://w3.org/TR/webrtc/#event-connectionstatechange
    pub fn on_connection_state_change<F>(&self, f: Option<F>)
    where
        F: 'static + FnMut(PeerConnectionState),
    {
        unimplemented!()
    }

    /// Adds remote [RTCPeerConnection][1]'s [ICE candidate][2] to this
    /// [`RtcPeerConnection`].
    ///
    /// # Errors
    ///
    /// With [`RtcPeerConnectionError::AddIceCandidateFailed`] if
    /// [RtcPeerConnection.addIceCandidate()][3] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://tools.ietf.org/html/rfc5245#section-2
    /// [3]: https://w3.org/TR/webrtc/#dom-peerconnection-addicecandidate
    pub async fn add_ice_candidate(
        &self,
        candidate: &str,
        sdp_m_line_index: Option<u16>,
        sdp_mid: &Option<String>,
    ) -> Result<()> {
        unimplemented!()
    }

    /// Marks [`RtcPeerConnection`] to trigger ICE restart.
    ///
    /// After this function returns, the offer returned by the next call to
    /// [`RtcPeerConnection::create_offer`] is automatically configured
    /// to trigger ICE restart.
    #[inline]
    pub fn restart_ice(&self) {
        unimplemented!()
    }

    /// Sets provided [SDP offer][`SdpType::Offer`] as local description.
    ///
    /// # Errors
    ///
    /// With [`RtcPeerConnectionError::SetLocalDescriptionFailed`] if
    /// [RtcPeerConnection.setLocalDescription()][1] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-peerconnection-setlocaldescription
    pub async fn set_offer(&self, offer: &str) -> Result<()> {
        unimplemented!()
    }

    /// Sets provided [SDP answer][`SdpType::Answer`] as local description.
    ///
    /// # Errors
    ///
    /// With [`RtcPeerConnectionError::SetLocalDescriptionFailed`] if
    /// [RtcPeerConnection.setLocalDescription()][1] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-peerconnection-setlocaldescription
    pub async fn set_answer(&self, answer: &str) -> Result<()> {
        unimplemented!()
    }

    /// Obtains [SDP answer][`SdpType::Answer`] from the underlying
    /// [RTCPeerConnection][`SysRtcPeerConnection`].
    ///
    /// Should be called whenever remote description has been changed.
    ///
    /// # Errors
    ///
    /// With [`RtcPeerConnectionError::CreateAnswerFailed`] if
    /// [RtcPeerConnection.createAnswer()][1] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection-createanswer
    pub async fn create_answer(&self) -> Result<String> {
        unimplemented!()
    }

    /// Rollbacks the underlying [RTCPeerConnection][`SysRtcPeerConnection`] to
    /// the previous stable state.
    ///
    /// # Errors
    ///
    /// With [`RtcPeerConnectionError::SetLocalDescriptionFailed`] if
    /// [RtcPeerConnection.setLocalDescription()][1] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-peerconnection-setlocaldescription
    pub async fn rollback(&self) -> Result<()> {
        unimplemented!()
    }

    /// Obtains [SDP offer][`SdpType::Offer`] from the underlying
    /// [RTCPeerConnection][`SysRtcPeerConnection`].
    ///
    /// Should be called after local tracks changes, which require
    /// (re)negotiation.
    ///
    /// # Errors
    ///
    /// With [`RtcPeerConnectionError::CreateOfferFailed`] if
    /// [RtcPeerConnection.createOffer()][1] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection-createoffer
    pub async fn create_offer(&self) -> Result<String> {
        unimplemented!()
    }

    /// Instructs the underlying [RTCPeerConnection][`SysRtcPeerConnection`]
    /// to apply the supplied [SDP][`SdpType`] as the remote
    /// [offer][`SdpType::Offer`] or [answer][`SdpType::Answer`].
    ///
    /// Changes the local media state.
    ///
    /// # Errors
    ///
    /// With [`RtcPeerConnectionError::SetRemoteDescriptionFailed`] if
    /// [RTCPeerConnection.setRemoteDescription()][1] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-peerconnection-setremotedescription
    pub async fn set_remote_description(&self, sdp: SdpType) -> Result<()> {
        unimplemented!()
    }

    /// Creates new [`RtcRtpTransceiver`] (see [RTCRtpTransceiver][1])
    /// and adds it to the [set of this RTCPeerConnection's transceivers][2].
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver
    /// [2]: https://w3.org/TR/webrtc/#transceivers-set
    #[must_use]
    pub fn add_transceiver(
        &self,
        kind: MediaKind,
        direction: TransceiverDirection,
    ) -> Transceiver {
        unimplemented!()
    }

    /// Returns [`RtcRtpTransceiver`] (see [RTCRtpTransceiver][1]) from a
    /// [set of this RTCPeerConnection's transceivers][2] by provided `mid`.
    ///
    /// # Panics
    ///
    /// If fails to [iterate over transceivers on JS side](js_sys::try_iter).
    /// Not supposed to ever happen.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver
    /// [2]: https://w3.org/TR/webrtc/#transceivers-set
    #[must_use]
    pub fn get_transceiver_by_mid(&self, mid: &str) -> Option<Transceiver> {
        unimplemented!()
    }
}
