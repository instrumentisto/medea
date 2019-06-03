use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::{
    future::{self, join_all, IntoFuture},
    sync::mpsc::UnboundedSender,
    Future,
};
use protocol::{Direction, IceCandidate as IceDto, MediaType, Track};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    MediaStreamTrack, RtcIceCandidateInit, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcRtpSender, RtcRtpTransceiver,
    RtcRtpTransceiverDirection, RtcRtpTransceiverInit, RtcSdpType,
    RtcSessionDescription, RtcSessionDescriptionInit, RtcTrackEvent,
};

use crate::{
    media::{stream::MediaStream, GetMediaRequest, MediaManager},
    utils::{EventListener, WasmErr},
};

pub type Id = u64;

pub enum Sdp {
    Offer(String),
    Answer(String),
}

#[allow(clippy::module_name_repetitions)]
pub enum PeerEvent {
    IceCandidateDiscovered {
        peer_id: Id,
        ice: IceDto,
    },
    NewRemoteStream {
        peer_id: Id,
        sender_id: u64,
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
        let peer = Rc::new(PeerConnection::new(id, &self.peer_events_sender)?);
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

struct MediaConnections {
    senders: HashMap<u64, Sender>,
    receivers: HashMap<u64, Receiver>,
}

// update Receiver associated with provided track by transceiver's mid, find
// track sender
impl MediaConnections {
    pub fn add_track(&mut self, track_event: &RtcTrackEvent) -> Option<u64> {
        let transceiver = track_event.transceiver();
        // should be safe to unwrap
        let mid = transceiver.mid().unwrap();

        let mut sender: Option<u64> = None;
        for receiver in &mut self.receivers.values_mut() {
            if let Some(recv_mid) = &receiver.mid {
                if recv_mid == &mid {
                    let track = track_event.track();
                    receiver.transceiver.replace(transceiver);
                    receiver.track.replace(track);
                    sender = Some(receiver.sender_id);
                    break;
                }
            }
        }

        sender
    }

    pub fn get_by_sender(
        &mut self,
        sender_id: u64,
    ) -> impl Iterator<Item = &mut Receiver> {
        self.receivers.iter_mut().filter_map(move |(_, receiver)| {
            if receiver.sender_id == sender_id {
                Some(receiver)
            } else {
                None
            }
        })
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection {
    peer: Rc<RtcPeerConnection>,
    _on_ice_candidate:
        EventListener<RtcPeerConnection, RtcPeerConnectionIceEvent>,
    _on_track: EventListener<RtcPeerConnection, RtcTrackEvent>,
    connections: Rc<RefCell<MediaConnections>>,
}

impl PeerConnection {
    pub fn new(
        peer_id: Id,
        peer_events_sender: &UnboundedSender<PeerEvent>,
    ) -> Result<Self, WasmErr> {
        let peer = Rc::new(RtcPeerConnection::new()?);
        let connections = Rc::new(RefCell::new(MediaConnections {
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }));

        let sender = peer_events_sender.clone();
        let on_ice_candidate = EventListener::new_mut(
            Rc::clone(&peer),
            "icecandidate",
            move |msg: RtcPeerConnectionIceEvent| {
                // TODO: examine None candidates, maybe we should send them
                if let Some(candidate) = msg.candidate() {
                    let _ = sender.unbounded_send(
                        PeerEvent::IceCandidateDiscovered {
                            peer_id,
                            ice: IceDto {
                                candidate: candidate.candidate(),
                                sdp_m_line_index: candidate.sdp_m_line_index(),
                                sdp_mid: candidate.sdp_mid(),
                            },
                        },
                    );
                }
            },
        )?;

        let sender = peer_events_sender.clone();
        let connections_rc = Rc::clone(&connections);
        let on_track = EventListener::new_mut(
            Rc::clone(&peer),
            "track",
            move |track_event: RtcTrackEvent| {
                let mut connections = connections_rc.borrow_mut();

                if let Some(sender_id) = connections.add_track(&track_event) {
                    let mut tracks: Vec<&MediaStreamTrack> = Vec::new();
                    for receiver in connections.get_by_sender(sender_id) {
                        match receiver.track {
                            None => return,
                            Some(ref track) => tracks.push(track),
                        }
                    }

                    // build and set MediaStream in receivers
                    let stream = MediaStream::from_tracks(&tracks);
                    for receiver in connections.get_by_sender(sender_id) {
                        let stream = Rc::clone(&stream);
                        receiver.stream.replace(stream);
                    }

                    let _ = sender.unbounded_send(PeerEvent::NewRemoteStream {
                        peer_id,
                        sender_id,
                        remote_stream: Rc::clone(&stream),
                    });
                }
            },
        )?;

        Ok(Self {
            peer,
            _on_ice_candidate: on_ice_candidate,
            _on_track: on_track,
            connections,
        })
    }

    pub fn get_mids(&self) -> Result<Option<HashMap<u64, String>>, WasmErr> {
        if self.connections.borrow().senders.is_empty() {
            return Ok(None);
        }

        let mut mids = HashMap::new();
        for (track, sender) in &self.connections.borrow().senders {
            mids.insert(
                *track,
                sender.transceiver.mid().ok_or_else(|| {
                    WasmErr::from_str("Peer has senders without mid")
                })?,
            );
        }

        Ok(Some(mids))
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

    pub fn update_tracks(
        &self,
        tracks: Vec<Track>,
        media_manager: &Rc<MediaManager>,
    ) -> impl Future<Item = (), Error = ()> {
        // insert peer transceivers
        for track in tracks {
            match track.direction {
                Direction::Send { .. } => {
                    self.connections.borrow_mut().senders.insert(
                        track.id,
                        Sender::new(&self.peer, track.media_type),
                    );
                }
                Direction::Recv { sender, mid } => {
                    self.connections
                        .borrow_mut()
                        .receivers
                        .insert(track.id, Receiver::new(sender, mid));
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
                            JsFuture::from(replace).map_err(WasmErr::from),
                        );
                    }
                    join_all(promises).map_err(|err| err.log_err())
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
    let senders = senders.into_iter();
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
    media_type: MediaType,
}

impl Sender {
    fn new(peer: &Rc<RtcPeerConnection>, media_type: MediaType) -> Self {
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
            media_type,
        }
    }
}

pub struct Receiver {
    transceiver: Option<RtcRtpTransceiver>,
    sender_id: u64,
    mid: Option<String>,
    track: Option<MediaStreamTrack>,
    stream: Option<Rc<MediaStream>>,
}

impl Receiver {
    fn new(sender_id: u64, mid: Option<String>) -> Self {
        Self {
            transceiver: None,
            sender_id,
            mid,
            track: None,
            stream: None,
        }
    }
}

impl Drop for PeerConnection {
    fn drop(&mut self) {
        self.peer.close()
    }
}
