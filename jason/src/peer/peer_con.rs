use futures::Future;
use medea_client_api_proto::IceServer;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    RtcConfiguration, RtcIceCandidateInit,
    RtcPeerConnection as SysRtcPeerConnection, RtcPeerConnectionIceEvent,
    RtcRtpTransceiver, RtcRtpTransceiverDirection, RtcRtpTransceiverInit,
    RtcSdpType, RtcSessionDescription, RtcSessionDescriptionInit,
    RtcTrackEvent,
};

use crate::{
    peer::ice_server::RtcIceServers,
    utils::{EventListener, WasmErr},
};

pub struct IceCandidate {
    pub candidate: String,
    pub sdp_m_line_index: Option<u16>,
    pub sdp_mid: Option<String>,
}

pub enum TransceiverType {
    Audio,
    Video,
}

pub enum TransceiverDirection {
    Sendonly,
    Recvonly,
}

pub enum SdpType {
    Offer(String),
    Answer(String),
}

struct InnerPeer {
    /// Underlying [`RtcPeerConnection`][1].
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    peer: Rc<SysRtcPeerConnection>,

    /// [`RtcPeerConnection`][1]s [`on_ice_candidate`][2] callback. Which
    /// fires when [`RtcPeerConnection`][1] discovers new ice candidate.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-onicecandidate
    on_ice_candidate:
        Option<EventListener<SysRtcPeerConnection, RtcPeerConnectionIceEvent>>,

    /// [`RtcPeerConnection`][1]'s [`on_track`][2] callback. Which fires when
    /// [`RtcPeerConnection`][1] receives new [`StreamTrack`] from remote
    /// peer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-ontrack
    on_track: Option<EventListener<SysRtcPeerConnection, RtcTrackEvent>>,
}

pub struct RtcPeerConnection(Rc<RefCell<InnerPeer>>);

impl RtcPeerConnection {
    pub fn new(ice_servers: Vec<IceServer>) -> Result<Self, WasmErr> {
        let mut peer_conf = RtcConfiguration::new();
        peer_conf.ice_servers(&RtcIceServers::from(ice_servers));

        Ok(Self(Rc::new(RefCell::new(InnerPeer {
            peer: Rc::new(SysRtcPeerConnection::new_with_configuration(
                &peer_conf,
            )?),
            on_ice_candidate: None,
            on_track: None,
        }))))
    }

    pub fn on_track<F>(&self, mut f: F) -> Result<(), WasmErr>
    where
        F: (FnMut(RtcTrackEvent)) + 'static,
    {
        let mut inner_mut = self.0.borrow_mut();
        inner_mut.on_track = Some(EventListener::new_mut(
            Rc::clone(&inner_mut.peer),
            "track",
            move |msg: RtcTrackEvent| {
                f(msg);
            },
        )?);
        Ok(())
    }

    pub fn on_ice_candidate<F>(&self, mut f: F) -> Result<(), WasmErr>
    where
        F: (FnMut(IceCandidate)) + 'static,
    {
        let mut inner_mut = self.0.borrow_mut();
        inner_mut.on_ice_candidate = Some(EventListener::new_mut(
            Rc::clone(&inner_mut.peer),
            "icecandidate",
            move |msg: RtcPeerConnectionIceEvent| {
                // TODO: examine None candidates, maybe we should send them
                //       (although no one does)
                if let Some(candidate) = msg.candidate() {
                    f(IceCandidate {
                        candidate: candidate.candidate(),
                        sdp_m_line_index: candidate.sdp_m_line_index(),
                        sdp_mid: candidate.sdp_mid(),
                    });
                }
            },
        )?);
        Ok(())
    }

    /// Adds remote [`RTCPeerConnection`][1]s [ICE Candidate][2] to this peer.
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

    /// Obtain SDP Answer from underlying [`RTCPeerConnection`][1] and set it as
    /// local description. Should be called whenever remote description is
    /// changed.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub fn create_and_set_answer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        let inner = self.0.borrow();

        let peer: Rc<SysRtcPeerConnection> = Rc::clone(&inner.peer);
        JsFuture::from(inner.peer.create_answer())
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

    /// Obtain SDP Offer from underlying [`RTCPeerConnection`][1] and set it as
    /// local description. Should be called after changing local tracks, but
    /// not all changes require renegotiation.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub fn create_and_set_offer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        let inner = self.0.borrow();

        let peer: Rc<SysRtcPeerConnection> = Rc::clone(&inner.peer);
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

    pub fn add_transceiver(
        &self,
        tr_type: &TransceiverType,
        direction: &TransceiverDirection,
    ) -> RtcRtpTransceiver {
        let mut init = RtcRtpTransceiverInit::new();
        match *direction {
            TransceiverDirection::Sendonly => {
                init.direction(RtcRtpTransceiverDirection::Sendonly)
            }
            TransceiverDirection::Recvonly => {
                init.direction(RtcRtpTransceiverDirection::Recvonly)
            }
        };

        match *tr_type {
            TransceiverType::Audio => self
                .0
                .borrow()
                .peer
                .add_transceiver_with_str_and_init("audio", &init),
            TransceiverType::Video => self
                .0
                .borrow()
                .peer
                .add_transceiver_with_str_and_init("video", &init),
        }
    }

    /// Find transceiver in peer transceivers by provided mid.
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
}

impl Drop for InnerPeer {
    fn drop(&mut self) {
        self.on_track.take();
        self.on_ice_candidate.take();
        self.peer.close()
    }
}
