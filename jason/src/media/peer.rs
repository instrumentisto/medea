use std::collections::HashMap;
use std::rc::Rc;
use web_sys::{
    RtcIceCandidateInit, RtcOfferOptions, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcRtpReceiver, RtcRtpSender,
    RtcRtpTransceiverDirection, RtcRtpTransceiverInit, RtcSdpType,
    RtcSessionDescription, RtcSessionDescriptionInit,
};

use crate::media::stream::MediaStream;
use crate::utils::{EventListener, WasmErr};
use futures::Future;
use protocol::{Direction, IceCandidate as IceDTO, MediaType, Track};
use std::cell::RefCell;
use wasm_bindgen_futures::JsFuture;
use crate::media::MediaManager;

pub type Id = u64;

pub enum Sdp {
    Offer(String),
    Answer(String),
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection(RefCell<InnerPeer>);

struct InnerPeer {
    id: Id,
    peer: Rc<RtcPeerConnection>,
    on_ice_candidate: RefCell<
        Option<EventListener<RtcPeerConnection, RtcPeerConnectionIceEvent>>,
    >,
    senders: HashMap<u64, RtcRtpSender>,
    receivers: HashMap<u64, RtcRtpReceiver>,
}

impl PeerConnection {
    pub fn new(id: Id) -> Result<Self, WasmErr> {
        Ok(Self(RefCell::new(InnerPeer {
            id,
            peer: Rc::new(RtcPeerConnection::new()?),
            on_ice_candidate: RefCell::new(None),
            senders: HashMap::default(),
            receivers: HashMap::default(),
        })))
    }

    pub fn create_and_set_offer(
        &self,
        receive_audio: bool,
        receive_video: bool,
        ice_restart: bool,
    ) -> impl Future<Item = String, Error = WasmErr> {
        let mut opts = RtcOfferOptions::new();
        opts.offer_to_receive_audio(receive_audio)
            .offer_to_receive_video(receive_video)
            .ice_restart(ice_restart);

        let inner = Rc::clone(&self.peer);
        JsFuture::from(self.peer.create_offer_with_rtc_offer_options(&opts))
            .map(RtcSessionDescription::from)
            .and_then(move |offer: RtcSessionDescription| {
                let offer = offer.sdp();
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                desc.sdp(&offer);
                JsFuture::from(inner.set_local_description(&desc))
                    .map(move |_| offer)
            })
            .map_err(Into::into)
    }

    pub fn create_and_set_answer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        let inner = Rc::clone(&self.peer);
        JsFuture::from(self.peer.create_answer())
            .map(RtcSessionDescription::from)
            .and_then(move |answer: RtcSessionDescription| {
                let answer = answer.sdp();
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                desc.sdp(&answer);
                JsFuture::from(inner.set_local_description(&desc))
                    .map(move |_| answer)
            })
            .map_err(Into::into)
    }

    pub fn set_remote_description(
        &self,
        sdp: Sdp,
    ) -> impl Future<Item = (), Error = WasmErr> {
        let desc = match sdp {
            Sdp::Offer(offer) => {
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                desc.sdp(&offer);
                desc
            }
            Sdp::Answer(answer) => {
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                desc.sdp(&answer);
                desc
            }
        };

        JsFuture::from(self.peer.set_remote_description(&desc))
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn add_ice_candidate(
        &self,
        candidate: &IceDTO,
    ) -> impl Future<Item = (), Error = WasmErr> {
        // TODO: According to Web IDL, return value is void.
        //       It may be worth to propose PR to wasm-bindgen, that would
        //       transform JsValue::Void to ().
        let mut cand_init = RtcIceCandidateInit::new(&candidate.candidate);
        cand_init
            .sdp_m_line_index(candidate.sdp_m_line_index)
            .sdp_mid(candidate.sdp_mid.as_ref().map(String::as_ref));
        JsFuture::from(
            self.peer.add_ice_candidate_with_opt_rtc_ice_candidate_init(
                Some(cand_init).as_ref(),
            ),
        )
        .map(|_| ())
        .map_err(Into::into)
    }

    pub fn add_stream(&self, stream: &Rc<MediaStream>) {
        self.peer.add_stream(&stream.get_media_stream());
    }

    pub fn on_remote_stream(&self) {}

    pub fn on_ice_candidate<F>(&self, mut f: F) -> Result<(), WasmErr>
    where
        F: (FnMut(IceDTO)) + 'static,
    {
        self.on_ice_candidate
            .borrow_mut()
            .replace(EventListener::new_mut(
                Rc::clone(&self.peer),
                "icecandidate",
                move |msg: RtcPeerConnectionIceEvent| {
                    // TODO: examine None candidates, maybe we should send them
                    if let Some(candidate) = msg.candidate() {
                        let candidate = IceDTO {
                            candidate: candidate.candidate(),
                            sdp_m_line_index: candidate.sdp_m_line_index(),
                            sdp_mid: candidate.sdp_mid(),
                        };

                        f(candidate);
                    }
                },
            )?);

        Ok(())
    }

    // TODO: properly cast and store all tracks/streams in PeerConnection
    //       with states (pending, active)
    pub fn sync_tracks(&self, tracks: Vec<Track>, media_manager: Rc<MediaManager>) {
        for track in tracks.into_iter() {
            let track_id = track.id;
            let track = (track.direction, track.media_type);

            let transceiver = match track {
                (Direction::Recv { sender }, MediaType::Audio(settings)) => {
                    let mut init = RtcRtpTransceiverInit::new();
                    init.direction(RtcRtpTransceiverDirection::Recvonly);
                    self.receivers.insert(
                        track_id,
                        self.peer
                            .add_transceiver_with_str_and_init("audio", &init)
                            .receiver(),
                    );
                }
                (Direction::Recv { sender }, MediaType::Video(settings)) => {
                    let mut init = RtcRtpTransceiverInit::new();
                    init.direction(RtcRtpTransceiverDirection::Recvonly);
                    self.receivers.insert(
                        track_id,
                        self.peer
                            .add_transceiver_with_str_and_init("video", &init)
                            .receiver(),
                    );
                }
                (Direction::Send { receivers }, MediaType::Audio(settings)) => {
                    let mut init = RtcRtpTransceiverInit::new();
                    init.direction(RtcRtpTransceiverDirection::Sendonly);
                    self.senders.insert(
                        track_id,
                        self.peer
                            .add_transceiver_with_str_and_init("audio", &init)
                            .sender(),
                    );
                }
                (Direction::Send { receivers }, MediaType::Video(settings)) => {
                    let mut init = RtcRtpTransceiverInit::new();
                    init.direction(RtcRtpTransceiverDirection::Sendonly);
                    self.senders.insert(
                        track_id,
                        self.peer
                            .add_transceiver_with_str_and_init("video", &init)
                            .sender(),
                    );
                }
            };
        }
    }
}

impl Drop for PeerConnection {
    fn drop(&mut self) {
        self.peer.close()
    }
}

#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct PeerRepository {
    peers: HashMap<Id, Rc<PeerConnection>>,
}

impl PeerRepository {
    // TODO: set ice_servers
    pub fn create(&mut self, id: Id) -> Result<&Rc<PeerConnection>, WasmErr> {
        let peer = Rc::new(PeerConnection::new(id)?);
        self.peers.insert(id, peer);
        Ok(self.peers.get(&id).unwrap())
    }

    pub fn get_peer(&self, id: Id) -> Option<&Rc<PeerConnection>> {
        self.peers.get(&id)
    }

    pub fn remove(&mut self, id: Id) {
        self.peers.remove(&id);
    }
}
