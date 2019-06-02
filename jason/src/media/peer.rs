use std::collections::HashMap;
use std::rc::Rc;
use web_sys::console;
use web_sys::{
    MediaStreamTrack, RtcIceCandidateInit, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcRtpSender, RtcRtpTransceiver,
    RtcRtpTransceiverDirection, RtcRtpTransceiverInit, RtcSdpType,
    RtcSessionDescription, RtcSessionDescriptionInit, RtcTrackEvent,
};

use crate::media::stream::MediaStream;
use crate::media::{GetMediaRequest, MediaManager};
use crate::utils::{EventListener, WasmErr};
use futures::future::join_all;
use futures::future::IntoFuture;
use futures::sync::mpsc::UnboundedSender;
use futures::{future, Future};
use protocol::{Direction, IceCandidate as IceDto, MediaType, Track};
use std::cell::RefCell;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

pub type Id = u64;

pub enum Sdp {
    Offer(String),
    Answer(String),
}

pub enum PeerEvent {
    IceCandidateDiscovered {
        peer_id: Id,
        ice: IceDto,
    },
    NewRemoteStream {
        peer_id: Id,
        remote_stream: Rc<MediaStream>,
    },
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerRepository {
    peers: HashMap<Id, Rc<PeerConnection>>,
    peer_events_sender: UnboundedSender<PeerEvent>,
}

impl PeerRepository {
    pub fn new(peer_events_sender: UnboundedSender<PeerEvent>) -> Self {
        Self {
            peers: HashMap::new(),
            peer_events_sender,
        }
    }

    // TODO: set ice_servers
    pub fn create(&mut self, id: Id) -> Result<&Rc<PeerConnection>, WasmErr> {
        let peer =
            Rc::new(PeerConnection::new(id, self.peer_events_sender.clone())?);
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

pub struct MediaConnections {
    senders: HashMap<u64, Sender>,
    receivers: HashMap<u64, Receiver>,
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection {
    id: Id,
    peer: Rc<RtcPeerConnection>,
    on_ice_candidate:
        EventListener<RtcPeerConnection, RtcPeerConnectionIceEvent>,
    on_track: EventListener<RtcPeerConnection, RtcTrackEvent>,
    connections: Rc<RefCell<MediaConnections>>,
    peer_events_sender: UnboundedSender<PeerEvent>,
}

impl PeerConnection {
    pub fn id(&self) -> Id {
        self.id
    }

    pub fn new(
        id: Id,
        peer_events_sender: UnboundedSender<PeerEvent>,
    ) -> Result<Self, WasmErr> {
        let peer = Rc::new(RtcPeerConnection::new()?);

        let sender = peer_events_sender.clone();
        let on_ice_candidate = EventListener::new_mut(
            Rc::clone(&peer),
            "icecandidate",
            move |msg: RtcPeerConnectionIceEvent| {
                // TODO: examine None candidates, maybe we should send them
                if let Some(candidate) = msg.candidate() {
                    sender.unbounded_send(PeerEvent::IceCandidateDiscovered {
                        peer_id: id,
                        ice: IceDto {
                            candidate: candidate.candidate(),
                            sdp_m_line_index: candidate.sdp_m_line_index(),
                            sdp_mid: candidate.sdp_mid(),
                        },
                    });
                }
            },
        )?;

        let connections = Rc::new(RefCell::new(MediaConnections {
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }));

        let sender = peer_events_sender.clone();
        let connections_rc = Rc::clone(&connections);
        let peer_rc = Rc::clone(&peer);
        let on_track = EventListener::new_mut(
            Rc::clone(&peer_rc),
            "track",
            move |new_track: RtcTrackEvent| {
                WasmErr::from_str("12345").log_err();
//                console::error_1(&new_track);
                console::error_1(&peer_rc.get_transceivers());



//                let receivers: &HashMap<u64, Receiver> =
//                    ;

//                for receiver in &connections_rc.borrow().receivers {
//                    WasmErr::from_str("asd").log_err();
//                    let track: MediaStreamTrack =
//                        receiver.1.transceiver.receiver().track();
//                    console::error_1(&receiver.1.transceiver);


//                }
            },
        )?;

        Ok(Self {
            id,
            peer,
            on_ice_candidate,
            on_track,
            connections,
            peer_events_sender,
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
        candidate: &IceDto,
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

    pub fn update_tracks(
        &self,
        tracks: Vec<Track>,
        media_manager: &Rc<MediaManager>,
    ) -> impl Future<Item = (), Error = ()> {
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
                .map_err(|err| err.log_err())
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
                        .map_err(|err| WasmErr::from(err).log_err())
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
    let mut audio = false;
    let mut video = false;

    senders.for_each(|s| match s.media_type {
        MediaType::Audio(_) => {
            audio = true;
        }
        MediaType::Video(_) => {
            video = true;
        }
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
