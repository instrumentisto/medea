use std::{cell::RefCell, rc::Rc};

use futures::Future;
use medea_client_api_proto::IceServer;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Event as SysEvent, RtcConfiguration, RtcIceCandidateInit,
    RtcPeerConnection as SysRtcPeerConnection, RtcPeerConnectionIceEvent,
    RtcRtpTransceiver, RtcRtpTransceiverDirection, RtcRtpTransceiverInit,
    RtcSdpType, RtcSessionDescription, RtcSessionDescriptionInit,
    RtcSignalingState, RtcTrackEvent,
};

use crate::utils::{EventListener, WasmErr};

use super::ice_server::RtcIceServers;

/// [RTCIceCandidate][1] representation.
///
/// [1]: https://www.w3.org/TR/webrtc/#rtcicecandidate-interface
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_m_line_index: Option<u16>,
    pub sdp_mid: Option<String>,
}

/// Representation of [RTCRtpTransceiver][1]'s [kind][2].
///
/// [1]: https://www.w3.org/TR/webrtc/#dom-rtcrtptransceiver
/// [2]: https://www.w3.org/TR/webrtc/#dfn-transceiver-kind
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum TransceiverKind {
    Audio,
    Video,
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
/// [1]:https://www.w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection
#[derive(Clone, Copy)]
// TODO: sendrecv optimization
pub enum TransceiverDirection {
    Sendonly,
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
/// [RTCSdpType]: https://www.w3.org/TR/webrtc/#dom-rtcsdptype
pub enum SdpType {
    Offer(String),
    Answer(String),
}

/// Helper wrapper for [RTCPeerConnection][1].
///
/// [1]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection
struct InnerPeer {
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
    on_ice_candidate:
        Option<EventListener<SysRtcPeerConnection, RtcPeerConnectionIceEvent>>,

    /// [`ontrack`][2] callback of [RTCPeerConnection][1] to handle
    /// [`track`][3] event. It fires when [RTCPeerConnection][1] receives
    /// new [MediaStreamTrack][4] from remote peer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-ontrack
    /// [3]: https://www.w3.org/TR/webrtc/#event-track
    /// [4]: https://developer.mozilla.org/en-US/docs/Web/API/MediaStreamTrack
    on_track: Option<EventListener<SysRtcPeerConnection, RtcTrackEvent>>,

    /// Event listener for [`signalingstatechange`].
    ///
    /// [`signalingstatechange`]: http://tiny.cc/6gbwcz
    on_signaling_state_change:
        Option<EventListener<SysRtcPeerConnection, SysEvent>>,
}

/// Representation of [RTCPeerConnection][1].
///
/// [1]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection
pub struct RtcPeerConnection(Rc<RefCell<InnerPeer>>);

impl RtcPeerConnection {
    /// Instantiates new [`RtcPeerConnection`].
    pub fn new<I>(ice_servers: I) -> Result<Self, WasmErr>
    where
        I: IntoIterator<Item = IceServer>,
    {
        // TODO: RTCBundlePolicy = "max-bundle"?
        let mut peer_conf = RtcConfiguration::new();
        peer_conf.ice_servers(&RtcIceServers::from(ice_servers));

        Ok(Self(Rc::new(RefCell::new(InnerPeer {
            peer: Rc::new(SysRtcPeerConnection::new_with_configuration(
                &peer_conf,
            )?),
            on_ice_candidate: None,
            on_track: None,
            on_signaling_state_change: None,
        }))))
    }

    /// Sets handler for [`RtcTrackEvent`] event (see [RTCTrackEvent][1] and
    /// [`ontrack` callback][2]).
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtctrackevent
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-ontrack
    pub fn on_track<F>(&self, f: Option<F>) -> Result<(), WasmErr>
    where
        F: 'static + FnMut(RtcTrackEvent),
    {
        let mut conn = self.0.borrow_mut();
        match f {
            None => conn.on_track = None,
            Some(mut f) => {
                conn.on_track = Some(EventListener::new_mut(
                    Rc::clone(&conn.peer),
                    "track",
                    move |msg: RtcTrackEvent| {
                        f(msg);
                    },
                )?);
            }
        }
        Ok(())
    }

    /// Sets handler for [`RtcPeerConnectionIceEvent`] event
    /// (see [RTCPeerConnectionIceEvent][1] and [`onicecandidate` callback][2]).
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnectioniceevent
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-onicecandidate
    pub fn on_ice_candidate<F>(&self, f: Option<F>) -> Result<(), WasmErr>
    where
        F: 'static + FnMut(IceCandidate),
    {
        let mut conn = self.0.borrow_mut();
        match f {
            None => conn.on_ice_candidate = None,
            Some(mut f) => {
                conn.on_ice_candidate = Some(EventListener::new_mut(
                    Rc::clone(&conn.peer),
                    "icecandidate",
                    move |msg: RtcPeerConnectionIceEvent| {
                        // TODO: examine None candidates, maybe we should send
                        //       them (although no one does)
                        if let Some(c) = msg.candidate() {
                            f(IceCandidate {
                                candidate: c.candidate(),
                                sdp_m_line_index: c.sdp_m_line_index(),
                                sdp_mid: c.sdp_mid(),
                            });
                        }
                    },
                )?);
            }
        }
        Ok(())
    }

    /// Sets handler for `signalingstatechange` event.
    pub fn on_signaling_state_changed<F>(
        &self,
        f: Option<F>,
    ) -> Result<(), WasmErr>
    where
        F: 'static + FnMut(),
    {
        let mut conn = self.0.borrow_mut();
        match f {
            None => conn.on_signaling_state_change = None,
            Some(mut f) => {
                conn.on_signaling_state_change = Some(EventListener::new_mut(
                    Rc::clone(&conn.peer),
                    "signalingstatechange",
                    move |_: SysEvent| {
                        f();
                    },
                )?)
            }
        }

        Ok(())
    }

    /// Adds remote [RTCPeerConnection][1]'s [ICE candidate][2] to this
    /// [`RtcPeerConnection`].
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://tools.ietf.org/html/rfc5245#section-2
    pub fn add_ice_candidate(
        &self,
        candidate: &str,
        sdp_m_line_index: Option<u16>,
        sdp_mid: &Option<String>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        let mut cand_init = RtcIceCandidateInit::new(&candidate);
        cand_init
            .sdp_m_line_index(sdp_m_line_index)
            .sdp_mid(sdp_mid.as_ref().map(String::as_ref));
        JsFuture::from(
            self.0
                .borrow()
                .peer
                .add_ice_candidate_with_opt_rtc_ice_candidate_init(
                    Some(cand_init).as_ref(),
                ),
        )
        .map(|_| ())
        .map_err(Into::into)
    }

    /// Obtains [SDP answer][`SdpType::Answer`] from the underlying
    /// [RTCPeerConnection][`SysRtcPeerConnection`] and sets it as a local
    /// description.
    ///
    /// Should be called whenever remote description has been changed.
    pub fn create_and_set_answer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        let conn = self.0.borrow();
        let peer: Rc<SysRtcPeerConnection> = Rc::clone(&conn.peer);
        JsFuture::from(conn.peer.create_answer())
            .map(RtcSessionDescription::from)
            .and_then(move |answer: RtcSessionDescription| {
                let answer = answer.sdp();
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                desc.sdp(&answer);
                JsFuture::from(peer.set_local_description(&desc))
                    .map(move |_| answer)
            })
            .map_err(Into::into)
    }

    /// Obtains [SDP offer][`SdpType::Offer`] from the underlying
    /// [RTCPeerConnection][`SysRtcPeerConnection`] and sets it as a local
    /// description.
    ///
    /// Should be called after local tracks changes, which require
    /// renegotiation.
    pub fn create_and_set_offer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        let conn = self.0.borrow();
        let peer: Rc<SysRtcPeerConnection> = Rc::clone(&conn.peer);
        JsFuture::from(peer.create_offer())
            .map(RtcSessionDescription::from)
            .and_then(move |offer: RtcSessionDescription| {
                let offer = offer.sdp();
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                desc.sdp(&offer);

                JsFuture::from(peer.set_local_description(&desc))
                    .map(move |_| offer)
            })
            .map_err(Into::into)
    }

    /// Instructs the underlying [RTCPeerConnection][`SysRtcPeerConnection`]
    /// to apply the supplied [SDP][`SdpType`] as the remote
    /// [offer][`SdpType::Offer`] or [answer][`SdpType::Answer`].
    ///
    /// Changes the local media state.
    pub fn set_remote_description(
        &self,
        sdp: SdpType,
    ) -> impl Future<Item = (), Error = WasmErr> {
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

        JsFuture::from(
            self.0.borrow().peer.set_remote_description(&description),
        )
        .map_err(Into::into)
        .map(|_| ())
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
        self.0
            .borrow()
            .peer
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

        let transceivers =
            js_sys::try_iter(&self.0.borrow().peer.get_transceivers())
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

    /// Returns current JS signaling state of [`RtcPeerConnection`].
    pub fn signaling_state(&self) -> RtcSignalingState {
        self.0.borrow().peer.signaling_state()
    }

    /// Returns current local [`RtcSessionDescription`].
    pub fn current_local_description(&self) -> Option<RtcSessionDescription> {
        self.0.borrow().peer.current_local_description()
    }

    /// Returns current remote [`RtcSessionDescription`].
    pub fn current_remote_description(&self) -> Option<RtcSessionDescription> {
        self.0.borrow().peer.current_remote_description()
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
        let mut inner = self.0.borrow_mut();
        inner.on_track.take();
        inner.on_ice_candidate.take();
        inner.on_signaling_state_change.take();
        inner.peer.close();
    }
}
