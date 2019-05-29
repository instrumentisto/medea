use std::collections::HashMap;
use std::rc::Rc;
use web_sys::{
    MediaStreamTrack, RtcIceCandidateInit, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcRtpSender, RtcRtpTransceiver,
    RtcRtpTransceiverDirection, RtcRtpTransceiverInit, RtcSdpType,
    RtcSessionDescription, RtcSessionDescriptionInit,
};

use crate::media::stream::MediaStream;
use crate::media::{GetMediaRequest, MediaManager};
use crate::utils::{EventListener, WasmErr};
use futures::future::join_all;
use futures::future::IntoFuture;
use futures::{future, Future};
use protocol::{Direction, IceCandidate as IceDTO, MediaType, Track};
use std::cell::RefCell;
use wasm_bindgen_futures::JsFuture;

pub type Id = u64;

pub enum Sdp {
    Offer(String),
    Answer(String),
}

pub struct MediaConnections {
    senders: HashMap<u64, Sender>,
    receivers: HashMap<u64, Receiver>,
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection {
    id: Id,
    peer: Rc<RtcPeerConnection>,
    on_ice_candidate: RefCell<
        Option<EventListener<RtcPeerConnection, RtcPeerConnectionIceEvent>>,
    >,
    connections: Rc<RefCell<MediaConnections>>,
}

impl PeerConnection {
    pub fn id(&self) -> Id {
        self.id
    }

    pub fn new(id: Id) -> Result<Self, WasmErr> {
        Ok(Self {
            id,
            peer: Rc::new(RtcPeerConnection::new()?),
            on_ice_candidate: RefCell::new(None),
            connections: Rc::new(RefCell::new(MediaConnections {
                senders: HashMap::new(),
                receivers: HashMap::new(),
            })),
        })
    }

    pub fn create_and_set_offer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        let inner = Rc::clone(&self.peer);
        JsFuture::from(self.peer.create_offer())
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

    pub fn update_tracks(
        &self,
        tracks: Vec<Track>,
        media_manager: &Rc<MediaManager>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        // insert peer transceivers
        for track in tracks {
            match track.direction {
                Direction::Send { receivers } => {
                    self.connections.borrow_mut().senders.insert(
                        track.id,
                        Sender::new(&self.peer, receivers, track.media_type),
                    );
                }
                Direction::Recv { sender } => {
                    self.connections.borrow_mut().receivers.insert(
                        track.id,
                        Receiver::new(&self.peer, sender, track.media_type),
                    );
                }
            }
        }
        // if senders not empty, then get media from media manager and insert
        // tracks into transceivers
        if self.connections.borrow().senders.is_empty() {
            future::Either::B(future::ok(()))
        } else {
            let media_request = build_get_media_request(
                self.connections.borrow().senders.values(),
            );

            let media_manager = Rc::clone(media_manager);
            let connections = Rc::clone(&self.connections);
            let get_media = media_request
                .into_future()
                .and_then(move |media_request| {
                    media_manager.get_stream(&media_request)
                })
                .and_then(move |stream: Rc<MediaStream>| {
                    let mut promises = Vec::new();
                    for sender in connections.borrow().senders.values() {
                        let rtc_sender: RtcRtpSender =
                            sender.transceiver.sender();
                        let replace = match sender.media_type {
                            MediaType::Audio(_) => rtc_sender
                                .replace_track(stream.get_audio_track()),
                            MediaType::Video(_) => rtc_sender
                                .replace_track(stream.get_video_track()),
                        };

                        promises.push(
                            JsFuture::from(replace)
                                .map_err(|err| WasmErr::from(err)),
                        );
                    }

                    join_all(promises)
                })
                .map(|_| ());

            future::Either::A(get_media)
        }
    }
}

/// Builds [`GetMediaRequest`] from peer senders. Currently allows only one
/// audio and one video track to be requested.
fn build_get_media_request<'a>(
    senders: impl IntoIterator<Item = &'a Sender>,
) -> Result<GetMediaRequest, WasmErr> {
    let mut senders = senders.into_iter();
    let audio = senders.any(|s| match s.media_type {
        MediaType::Audio(_) => true,
        MediaType::Video(_) => false,
    });

    let video = senders.any(|s| match s.media_type {
        MediaType::Audio(_) => false,
        MediaType::Video(_) => true,
    });

    GetMediaRequest::new(audio, video)
}

pub struct Sender {
    transceiver: RtcRtpTransceiver,
    receivers: Vec<u64>,
    media_type: MediaType,
    track: Option<MediaStreamTrack>,
    stream: Option<Rc<MediaStream>>,
}

impl Sender {
    fn new(
        peer: &Rc<RtcPeerConnection>,
        receivers: Vec<u64>,
        media_type: MediaType,
    ) -> Self {
        let transceiver = match media_type {
            MediaType::Audio(_) => {
                let mut init = RtcRtpTransceiverInit::new();
                init.direction(RtcRtpTransceiverDirection::Sendonly);
                peer.add_transceiver_with_str_and_init("audio", &init)
            }
            MediaType::Video(_) => {
                let mut init = RtcRtpTransceiverInit::new();
                init.direction(RtcRtpTransceiverDirection::Sendonly);
                peer.add_transceiver_with_str_and_init("video", &init)
            }
        };

        Self {
            transceiver,
            receivers,
            media_type,
            track: None,
            stream: None,
        }
    }
}

pub struct Receiver {
    transceiver: RtcRtpTransceiver,
    sender: u64,
    media_type: MediaType,
    track: Option<MediaStreamTrack>,
}

impl Receiver {
    fn new(
        peer: &Rc<RtcPeerConnection>,
        sender: u64,
        media_type: MediaType,
    ) -> Self {
        let transceiver = match media_type {
            MediaType::Audio(_) => {
                let mut init = RtcRtpTransceiverInit::new();
                init.direction(RtcRtpTransceiverDirection::Recvonly);
                peer.add_transceiver_with_str_and_init("audio", &init)
            }
            MediaType::Video(_) => {
                let mut init = RtcRtpTransceiverInit::new();
                init.direction(RtcRtpTransceiverDirection::Recvonly);
                peer.add_transceiver_with_str_and_init("video", &init)
            }
        };

        Self {
            transceiver,
            sender,
            media_type,
            track: None,
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
