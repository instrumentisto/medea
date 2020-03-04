use std::{cell::RefCell, rc::Rc};

use derive_more::Display;
use medea_client_api_proto::{
    Direction as DirectionProto, IceServer, PeerConnectionState,
};
use tracerr::Traced;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Event, RtcBundlePolicy, RtcConfiguration, RtcIceCandidateInit,
    RtcIceConnectionState, RtcIceTransportPolicy,
    RtcPeerConnection as SysRtcPeerConnection, RtcPeerConnectionIceEvent,
    RtcRtpTransceiver, RtcRtpTransceiverDirection, RtcRtpTransceiverInit,
    RtcSdpType, RtcSessionDescription, RtcSessionDescriptionInit,
    RtcTrackEvent,
};

use crate::{
    media::TrackConstraints,
    utils::{
        console_error, get_property_by_name, EventListener,
        EventListenerBindError, JsCaused, JsError,
    },
};

use super::ice_server::RtcIceServers;

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

/// Representation of [RTCRtpTransceiver][1]'s [kind][2].
///
/// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver
/// [2]: https://w3.org/TR/webrtc/#dfn-transceiver-kind
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum TransceiverKind {
    /// Audio transceiver.
    Audio,

    /// Video transceiver.
    Video,
}

impl From<&TrackConstraints> for TransceiverKind {
    fn from(media_type: &TrackConstraints) -> Self {
        match media_type {
            TrackConstraints::Audio(_) => Self::Audio,
            TrackConstraints::Video(_) => Self::Video,
        }
    }
}

impl TransceiverKind {
    /// Returns string representation of a [`TransceiverKind`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Video => "video",
        }
    }
}

/// Representation of [RTCRtpTransceiverDirection][1].
///
/// [1]:https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection
#[derive(Clone, Copy)]
// TODO: sendrecv optimization
pub enum TransceiverDirection {
    /// [`sendonly` direction][1] of transceiver.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection-sendonly
    Sendonly,

    /// [`recvonly` direction][1] of transceiver.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection-recvonly
    Recvonly,
}

impl From<TransceiverDirection> for RtcRtpTransceiverDirection {
    fn from(direction: TransceiverDirection) -> Self {
        use TransceiverDirection::*;
        match direction {
            Sendonly => Self::Sendonly,
            Recvonly => Self::Recvonly,
        }
    }
}

impl From<&DirectionProto> for TransceiverDirection {
    #[inline]
    fn from(proto: &DirectionProto) -> Self {
        match proto {
            DirectionProto::Recv { .. } => Self::Recvonly,
            DirectionProto::Send { .. } => Self::Sendonly,
        }
    }
}

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

/// Errors that may occur during signaling between this and remote
/// [RTCPeerConnection][1] and event handlers setting errors.
///
/// [1]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection.
#[derive(Debug, Display, JsCaused)]
pub enum RTCPeerConnectionError {
    /// Occurs when cannot adds new remote candidate to the
    /// [RTCPeerConnection][1]'s remote description.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection.
    #[display(fmt = "Failed to add ICE candidate: {}", _0)]
    AddIceCandidateFailed(JsError),

    /// Occurs when cannot obtains [SDP answer][`SdpType::Answer`] from
    /// the underlying [RTCPeerConnection][`SysRtcPeerConnection`].
    #[display(fmt = "Failed to create SDP answer: {}", _0)]
    CreateAnswerFailed(JsError),

    /// Occurs when a new [`RtcPeerConnection`] cannot be created.
    #[display(fmt = "Failed to create PeerConnection: {}", _0)]
    PeerCreationError(JsError),

    /// Occurs when cannot obtains [SDP offer][`SdpType::Offer`] from
    /// the underlying [RTCPeerConnection][`SysRtcPeerConnection`]
    #[display(fmt = "Failed to create SDP offer: {}", _0)]
    CreateOfferFailed(JsError),

    /// Occurs when handler failed to bind to some [`RtcPeerConnection`] event.
    /// Not really supposed to ever happen.
    #[display(fmt = "Failed to bind to RTCPeerConnection event: {}", _0)]
    PeerConnectionEventBindFailed(EventListenerBindError),

    /// Occurs if the local description associated with the
    /// [`RtcPeerConnection`] cannot be changed.
    #[display(fmt = "Failed to set local SDP description: {}", _0)]
    SetLocalDescriptionFailed(JsError),

    /// Occurs if the description of the remote end of the
    /// [`RtcPeerConnection`] cannot be changed.
    #[display(fmt = "Failed to set remote SDP description: {}", _0)]
    SetRemoteDescriptionFailed(JsError),
}

type Result<T> = std::result::Result<T, Traced<RTCPeerConnectionError>>;

impl From<EventListenerBindError> for RTCPeerConnectionError {
    fn from(err: EventListenerBindError) -> Self {
        Self::PeerConnectionEventBindFailed(err)
    }
}

/// Representation of [RTCPeerConnection][1].
///
/// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
pub struct RtcPeerConnection {
    /// Underlying [RTCPeerConnection][1].
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    peer: Rc<SysRtcPeerConnection>,

    /// [`onicecandidate`][2] callback of [RTCPeerConnection][1] to handle
    /// [`icecandidate`][3] event. It fires when [RTCPeerConnection][1]
    /// discovers a new [RTCIceCandidate][4].
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-onicecandidate
    /// [3]: https://www.w3.org/TR/webrtc/#event-icecandidate
    /// [4]: https://www.w3.org/TR/webrtc/#dom-rtcicecandidate
    on_ice_candidate: RefCell<
        Option<EventListener<SysRtcPeerConnection, RtcPeerConnectionIceEvent>>,
    >,

    /// [`iceconnectionstatechange`][2] callback of [RTCPeerConnection][1],
    /// fires whenever [ICE connection state][3] changes.
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://w3.org/TR/webrtc/#event-iceconnectionstatechange
    /// [3]: https://w3.org/TR/webrtc/#dfn-ice-connection-state
    on_ice_connection_state_changed:
        RefCell<Option<EventListener<SysRtcPeerConnection, Event>>>,

    /// [`connectionstatechange`][2] callback of [RTCPeerConnection][1],
    /// fires whenever the aggregate state of the connection changes.
    /// The aggregate state is a combination of the states of all individual
    /// network transports being used by the connection.
    ///
    /// Implemented in Chrome and Safari.
    /// Tracking issue for Firefox:
    /// <https://bugzilla.mozilla.org/show_bug.cgi?id=1265827>
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/webrtc/#event-connectionstatechange
    on_connection_state_changed:
        RefCell<Option<EventListener<SysRtcPeerConnection, Event>>>,

    /// [`ontrack`][2] callback of [RTCPeerConnection][1] to handle
    /// [`track`][3] event. It fires when [RTCPeerConnection][1] receives
    /// new [MediaStreamTrack][4] from remote peer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-ontrack
    /// [3]: https://www.w3.org/TR/webrtc/#event-track
    /// [4]: https://developer.mozilla.org/en-US/docs/Web/API/MediaStreamTrack
    on_track:
        RefCell<Option<EventListener<SysRtcPeerConnection, RtcTrackEvent>>>,
}

impl RtcPeerConnection {
    /// Instantiates new [`RtcPeerConnection`].
    ///
    /// # Errors
    ///
    /// Errors with [`RTCPeerConnectionError::PeerCreationError`] if
    /// [`SysRtcPeerConnection`] creation fails.
    pub fn new<I>(ice_servers: I, is_force_relayed: bool) -> Result<Self>
    where
        I: IntoIterator<Item = IceServer>,
    {
        let mut peer_conf = RtcConfiguration::new();
        let policy = if is_force_relayed {
            RtcIceTransportPolicy::Relay
        } else {
            RtcIceTransportPolicy::All
        };
        peer_conf.bundle_policy(RtcBundlePolicy::MaxBundle);
        peer_conf.ice_transport_policy(policy);
        peer_conf.ice_servers(&RtcIceServers::from(ice_servers));
        let peer = SysRtcPeerConnection::new_with_configuration(&peer_conf)
            .map_err(Into::into)
            .map_err(RTCPeerConnectionError::PeerCreationError)
            .map_err(tracerr::wrap!())?;

        Ok(Self {
            peer: Rc::new(peer),
            on_ice_candidate: RefCell::new(None),
            on_ice_connection_state_changed: RefCell::new(None),
            on_connection_state_changed: RefCell::new(None),
            on_track: RefCell::new(None),
        })
    }

    /// Sets handler for [`RtcTrackEvent`] event (see [RTCTrackEvent][1] and
    /// [`ontrack` callback][2]).
    ///
    /// # Errors
    ///
    /// Errors with [`RTCPeerConnectionError::PeerConnectionEventBindFailed`] if
    /// [`EventListener`] binding fails.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtctrackevent
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-ontrack
    pub fn on_track<F>(&self, f: Option<F>) -> Result<()>
    where
        F: 'static + FnMut(RtcTrackEvent),
    {
        let mut on_track = self.on_track.borrow_mut();
        match f {
            None => {
                on_track.take();
            }
            Some(mut f) => {
                on_track.replace(
                    EventListener::new_mut(
                        Rc::clone(&self.peer),
                        "track",
                        move |msg: RtcTrackEvent| {
                            f(msg);
                        },
                    )
                    .map_err(tracerr::map_from_and_wrap!())?,
                );
            }
        }
        Ok(())
    }

    /// Sets handler for [`RtcPeerConnectionIceEvent`] event
    /// (see [RTCPeerConnectionIceEvent][1] and [`onicecandidate` callback][2]).
    ///
    /// # Errors
    ///
    /// Errors with [`RTCPeerConnectionError::PeerConnectionEventBindFailed`] if
    /// [`EventListener`] binding fails.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnectioniceevent
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-onicecandidate
    pub fn on_ice_candidate<F>(&self, f: Option<F>) -> Result<()>
    where
        F: 'static + FnMut(IceCandidate),
    {
        let mut on_ice_candidate = self.on_ice_candidate.borrow_mut();
        match f {
            None => {
                on_ice_candidate.take();
            }
            Some(mut f) => {
                on_ice_candidate.replace(
                    EventListener::new_mut(
                        Rc::clone(&self.peer),
                        "icecandidate",
                        move |msg: RtcPeerConnectionIceEvent| {
                            // None candidate means that all ICE transports have
                            // finished gathering candidates.
                            // Doesn't need to be delivered onward to the remote
                            // peer.
                            if let Some(c) = msg.candidate() {
                                f(IceCandidate {
                                    candidate: c.candidate(),
                                    sdp_m_line_index: c.sdp_m_line_index(),
                                    sdp_mid: c.sdp_mid(),
                                });
                            }
                        },
                    )
                    .map_err(tracerr::map_from_and_wrap!())?,
                );
            }
        }
        Ok(())
    }

    /// Sets handler for [`iceconnectionstatechange`][1] event.
    ///
    /// # Errors
    ///
    /// Will return [`RTCPeerConnectionError::PeerConnectionEventBindFailed`] if
    /// [`EventListener`] binding fails.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#event-iceconnectionstatechange
    pub fn on_ice_connection_state_change<F>(&self, f: Option<F>) -> Result<()>
    where
        F: 'static + FnMut(RtcIceConnectionState),
    {
        let mut on_ice_connection_state_changed =
            self.on_ice_connection_state_changed.borrow_mut();
        match f {
            None => {
                on_ice_connection_state_changed.take();
            }
            Some(mut f) => {
                let peer = Rc::clone(&self.peer);
                on_ice_connection_state_changed.replace(
                    EventListener::new_mut(
                        Rc::clone(&self.peer),
                        "iceconnectionstatechange",
                        move |_| {
                            f(peer.ice_connection_state());
                        },
                    )
                    .map_err(tracerr::map_from_and_wrap!())?,
                );
            }
        }
        Ok(())
    }

    /// Sets handler for [`connectionstatechange`][1] event.
    ///
    /// # Errors
    ///
    /// Will return [`RTCPeerConnectionError::PeerConnectionEventBindFailed`] if
    /// [`EventListener`] binding fails.
    /// This error can be ignored, since this event is currently implemented
    /// only in Chrome and Safari.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#event-connectionstatechange
    pub fn on_connection_state_change<F>(&self, f: Option<F>) -> Result<()>
    where
        F: 'static + FnMut(PeerConnectionState),
    {
        let mut on_connection_state_changed =
            self.on_connection_state_changed.borrow_mut();
        match f {
            None => {
                on_connection_state_changed.take();
            }
            Some(mut f) => {
                let peer = Rc::clone(&self.peer);
                on_connection_state_changed.replace(
                    EventListener::new_mut(
                        Rc::clone(&self.peer),
                        "connectionstatechange",
                        move |_| {
                            // Error here should never happen, because if the
                            // browser does not support the functionality of
                            // `RTCPeerConnection.connectionState`, then this
                            // callback won't fire.
                            if let Some(state) =
                                get_peer_connection_state(&peer)
                            {
                                let state = match state.as_ref() {
                                    "new" => PeerConnectionState::New,
                                    "connecting" => {
                                        PeerConnectionState::Connecting
                                    }
                                    "connected" => {
                                        PeerConnectionState::Connected
                                    }
                                    "disconnected" => {
                                        PeerConnectionState::Disconnected
                                    }
                                    "failed" => PeerConnectionState::Failed,
                                    "closed" => PeerConnectionState::Closed,
                                    _ => {
                                        console_error(format!(
                                            "Unknown RTCPeerConnection \
                                             connection state: {}.",
                                            state,
                                        ));
                                        return;
                                    }
                                };
                                f(state);
                            } else {
                                console_error(
                                    "Could not receive RTCPeerConnection \
                                     connection state",
                                );
                            }
                        },
                    )
                    .map_err(tracerr::map_from_and_wrap!())?,
                );
            }
        }
        Ok(())
    }

    /// Adds remote [RTCPeerConnection][1]'s [ICE candidate][2] to this
    /// [`RtcPeerConnection`].
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://tools.ietf.org/html/rfc5245#section-2
    pub async fn add_ice_candidate(
        &self,
        candidate: &str,
        sdp_m_line_index: Option<u16>,
        sdp_mid: &Option<String>,
    ) -> Result<()> {
        let mut cand_init = RtcIceCandidateInit::new(&candidate);
        cand_init
            .sdp_m_line_index(sdp_m_line_index)
            .sdp_mid(sdp_mid.as_ref().map(String::as_ref));
        JsFuture::from(
            self.peer.add_ice_candidate_with_opt_rtc_ice_candidate_init(
                Some(cand_init).as_ref(),
            ),
        )
        .await
        .map_err(Into::into)
        .map_err(RTCPeerConnectionError::AddIceCandidateFailed)
        .map_err(tracerr::wrap!())?;
        Ok(())
    }

    /// Obtains [SDP answer][`SdpType::Answer`] from the underlying
    /// [RTCPeerConnection][`SysRtcPeerConnection`] and sets it as a local
    /// description.
    ///
    /// Should be called whenever remote description has been changed.
    pub async fn create_and_set_answer(&self) -> Result<String> {
        let peer: Rc<SysRtcPeerConnection> = Rc::clone(&self.peer);

        let answer = JsFuture::from(self.peer.create_answer())
            .await
            .map_err(Into::into)
            .map_err(RTCPeerConnectionError::CreateAnswerFailed)
            .map_err(tracerr::wrap!())?;
        let answer = RtcSessionDescription::from(answer).sdp();

        let mut desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        desc.sdp(&answer);

        JsFuture::from(peer.set_local_description(&desc))
            .await
            .map_err(Into::into)
            .map_err(RTCPeerConnectionError::SetLocalDescriptionFailed)
            .map_err(tracerr::wrap!())?;

        Ok(answer)
    }

    /// Obtains [SDP offer][`SdpType::Offer`] from the underlying
    /// [RTCPeerConnection][`SysRtcPeerConnection`] and sets it as a local
    /// description.
    ///
    /// Should be called after local tracks changes, which require
    /// renegotiation.
    pub async fn create_and_set_offer(&self) -> Result<String> {
        let peer: Rc<SysRtcPeerConnection> = Rc::clone(&self.peer);

        let create_offer = JsFuture::from(peer.create_offer())
            .await
            .map_err(Into::into)
            .map_err(RTCPeerConnectionError::CreateOfferFailed)
            .map_err(tracerr::wrap!())?;
        let offer = RtcSessionDescription::from(create_offer).sdp();

        let mut desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        desc.sdp(&offer);

        JsFuture::from(peer.set_local_description(&desc))
            .await
            .map_err(Into::into)
            .map_err(RTCPeerConnectionError::SetLocalDescriptionFailed)
            .map_err(tracerr::wrap!())?;

        Ok(offer)
    }

    /// Instructs the underlying [RTCPeerConnection][`SysRtcPeerConnection`]
    /// to apply the supplied [SDP][`SdpType`] as the remote
    /// [offer][`SdpType::Offer`] or [answer][`SdpType::Answer`].
    ///
    /// Changes the local media state.
    pub async fn set_remote_description(&self, sdp: SdpType) -> Result<()> {
        let description = match sdp {
            SdpType::Offer(offer) => {
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                desc.sdp(&offer);
                desc
            }
            SdpType::Answer(answer) => {
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                desc.sdp(&answer);
                desc
            }
        };

        JsFuture::from(self.peer.set_remote_description(&description))
            .await
            .map_err(Into::into)
            .map_err(RTCPeerConnectionError::SetRemoteDescriptionFailed)
            .map_err(tracerr::wrap!())?;

        Ok(())
    }

    /// Creates new [`RtcRtpTransceiver`] (see [RTCRtpTransceiver][1])
    /// and adds it to the [set of this RTCPeerConnection's transceivers][2].
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcrtptransceiver
    /// [2]: https://www.w3.org/TR/webrtc/#transceivers-set
    pub fn add_transceiver(
        &self,
        kind: TransceiverKind,
        direction: TransceiverDirection,
    ) -> RtcRtpTransceiver {
        let mut init = RtcRtpTransceiverInit::new();
        init.direction(direction.into());
        self.peer
            .add_transceiver_with_str_and_init(kind.as_str(), &init)
    }

    /// Returns [`RtcRtpTransceiver`] (see [RTCRtpTransceiver][1]) from a
    /// [set of this RTCPeerConnection's transceivers][2] by provided `mid`.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcrtptransceiver
    /// [2]: https://www.w3.org/TR/webrtc/#transceivers-set
    pub fn get_transceiver_by_mid(
        &self,
        mid: &str,
    ) -> Option<RtcRtpTransceiver> {
        let mut transceiver = None;

        let transceivers = js_sys::try_iter(&self.peer.get_transceivers())
            .unwrap()
            .unwrap();
        for tr in transceivers {
            let tr = RtcRtpTransceiver::from(tr.unwrap());
            if let Some(tr_mid) = tr.mid() {
                if mid.eq(&tr_mid) {
                    transceiver = Some(tr);
                    break;
                }
            }
        }

        transceiver
    }
}

impl Drop for RtcPeerConnection {
    /// Drops [`on_track`][`RtcPeerConnection::on_track`] and
    /// [`on_ice_candidate`][`RtcPeerConnection::on_ice_candidate`] callbacks,
    /// and [closes][1] the underlying
    /// [RTCPeerConnection][`SysRtcPeerConnection`].
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-close
    fn drop(&mut self) {
        self.on_track.borrow_mut().take();
        self.on_ice_candidate.borrow_mut().take();
        self.on_ice_connection_state_changed.borrow_mut().take();
        self.on_connection_state_changed.borrow_mut().take();
        self.peer.close();
    }
}

/// Returns [RTCPeerConnection.connectionState][1] property of provided
/// [`SysRtcPeerConnection`] using reflection.
///
/// [1]: https://www.w3.org/TR/webrtc/#dom-peerconnection-connection-state
fn get_peer_connection_state(peer: &SysRtcPeerConnection) -> Option<String> {
    get_property_by_name(peer, "connectionState", |v| v.as_string())
}
