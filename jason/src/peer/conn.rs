use std::{cell::RefCell, rc::Rc};

use derive_more::Display;
use medea_client_api_proto::IceServer;
use tracerr::Traced;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    RtcConfiguration, RtcIceCandidateInit,
    RtcPeerConnection as SysRtcPeerConnection, RtcPeerConnectionIceEvent,
    RtcRtpTransceiver, RtcRtpTransceiverDirection, RtcRtpTransceiverInit,
    RtcSdpType, RtcSessionDescription, RtcSessionDescriptionInit,
    RtcTrackEvent,
};

use crate::{
    media::TrackConstraints,
    utils::{EventListener, JsCaused, JsError},
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
    #[display(fmt = "failed to add ICE candidate: {}", _0)]
    AddIceCandidate(JsError),

    /// Occurs when cannot obtains [SDP answer][`SdpType::Answer`] from
    /// the underlying [RTCPeerConnection][`SysRtcPeerConnection`].
    #[display(fmt = "failed to create SDP answer: {}", _0)]
    CreateAnswer(JsError),

    /// Occurs when a new [`RtcPeerConnection`] cannot be created.
    #[display(fmt = "failed to create PeerConnection: {}", _0)]
    CreatePeer(JsError),

    /// Occurs when cannot obtains [SDP offer][`SdpType::Offer`] from
    /// the underlying [RTCPeerConnection][`SysRtcPeerConnection`]
    #[display(fmt = "failed to create SDP offer: {}", _0)]
    CreateOffer(JsError),

    /// Occurs when handler cannot be set for the event
    /// [`RtcPeerConnectionIceEvent`].
    #[display(
        fmt = "failed to set handler for RtcPeerConnectionIceEvent: {}",
        _0
    )]
    SetHandlerIceEvent(JsError),

    /// Occurs when handler cannot be set for the event [`RtcTrackEvent`].
    #[display(fmt = "failed to set handler for RtcTrackEvent: {}", _0)]
    SetHandlerTrackEvent(JsError),

    /// Occurs if the local description associated with the
    /// [`RTCPeerConnection`] cannot be changed.
    #[display(fmt = "failed to set local SDP description: {}", _0)]
    SetLocalDescription(JsError),

    /// Occurs if the description of the remote end of the
    /// [`RTCPeerConnection`] cannot be changed.
    #[display(fmt = "failed to set remote SDP description: {}", _0)]
    SetRemoteDescription(JsError),
}

type Result<T> = std::result::Result<T, Traced<RTCPeerConnectionError>>;

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
    pub fn new<I>(ice_servers: I) -> Result<Self>
    where
        I: IntoIterator<Item = IceServer>,
    {
        // TODO: RTCBundlePolicy = "max-bundle"?
        let mut peer_conf = RtcConfiguration::new();
        peer_conf.ice_servers(&RtcIceServers::from(ice_servers));
        let peer = SysRtcPeerConnection::new_with_configuration(&peer_conf)
            .map_err(Into::into)
            .map_err(RTCPeerConnectionError::CreatePeer)
            .map_err(tracerr::wrap!())?;

        Ok(Self {
            peer: Rc::new(peer),
            on_ice_candidate: RefCell::new(None),
            on_track: RefCell::new(None),
        })
    }

    /// Sets handler for [`RtcTrackEvent`] event (see [RTCTrackEvent][1] and
    /// [`ontrack` callback][2]).
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
                    .map_err(RTCPeerConnectionError::SetHandlerTrackEvent)
                    .map_err(tracerr::wrap!())?,
                );
            }
        }
        Ok(())
    }

    /// Sets handler for [`RtcPeerConnectionIceEvent`] event
    /// (see [RTCPeerConnectionIceEvent][1] and [`onicecandidate` callback][2]).
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
                    .map_err(RTCPeerConnectionError::SetHandlerIceEvent)
                    .map_err(tracerr::wrap!())?,
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
        .map_err(RTCPeerConnectionError::AddIceCandidate)
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
            .map_err(RTCPeerConnectionError::CreateAnswer)
            .map_err(tracerr::wrap!())?;
        let answer = RtcSessionDescription::from(answer).sdp();

        let mut desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        desc.sdp(&answer);

        JsFuture::from(peer.set_local_description(&desc))
            .await
            .map_err(Into::into)
            .map_err(RTCPeerConnectionError::SetLocalDescription)
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
            .map_err(RTCPeerConnectionError::CreateOffer)
            .map_err(tracerr::wrap!())?;
        let offer = RtcSessionDescription::from(create_offer).sdp();

        let mut desc = RtcSessionDescriptionInit::new(RtcSdpType::Offer);
        desc.sdp(&offer);

        JsFuture::from(peer.set_local_description(&desc))
            .await
            .map_err(Into::into)
            .map_err(RTCPeerConnectionError::SetLocalDescription)
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
            .map_err(RTCPeerConnectionError::SetRemoteDescription)
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
        self.peer.close();
    }
}
