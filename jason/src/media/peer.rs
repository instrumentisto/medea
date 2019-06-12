use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::{
    future::{self, join_all},
    sync::mpsc::UnboundedSender,
    Future,
};
use medea_client_api_proto::{Direction, MediaType, Track};
use medea_macro::dispatchable;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    RtcIceCandidateInit, RtcPeerConnection, RtcPeerConnectionIceEvent,
    RtcRtpSender, RtcRtpTransceiver, RtcRtpTransceiverDirection,
    RtcRtpTransceiverInit, RtcSdpType, RtcSessionDescription,
    RtcSessionDescriptionInit, RtcTrackEvent,
};

use crate::{
    media::{
        stream::MediaStream, track::MediaTrack, MediaManager, StreamRequest,
    },
    utils::{EventListener, WasmErr},
};
use futures::future::Either;

pub type Id = u64;

pub enum Sdp {
    Offer(String),
    Answer(String),
}

#[dispatchable]
#[allow(clippy::module_name_repetitions)]
/// Events emitted from [`PeerConnection`].
pub enum PeerEvent {
    /// [`PeerConnection`] discovered new ice candidate.
    IceCandidateDiscovered {
        peer_id: Id,
        candidate: String,
        sdp_m_line_index: Option<u16>,
        sdp_mid: Option<String>,
    },

    /// [`PeerConnection`] received new stream from remote sender.
    NewRemoteStream {
        peer_id: Id,
        sender_id: u64,
        remote_stream: Rc<MediaStream>,
    },
}

///
#[allow(clippy::module_name_repetitions)]
pub struct PeerRepository {
    /// Peer id to [`PeerConnection`],
    peers: HashMap<Id, Rc<PeerConnection>>,

    /// Sender that will be injected to all [`Peers`] created by this
    /// repository.
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

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection {
    /// Underlying [`RtcPeerConnection`].
    peer: Rc<RtcPeerConnection>,

    /// [`RtcPeerConnection`]'s [`on_ice_candidate`] callback. Which fires when
    /// [`RtcPeerConnection`] discovers new ice candidate.
    _on_ice_candidate:
        EventListener<RtcPeerConnection, RtcPeerConnectionIceEvent>,

    /// [`RtcPeerConnection`]'s [`_on_track`] callback. Which fires when
    /// [`RtcPeerConnection`] receives new [`StreamTrack`] from remote
    /// peer.
    _on_track: EventListener<RtcPeerConnection, RtcTrackEvent>,
    media_connections: Rc<RefCell<MediaConnections>>,
}

impl PeerConnection {
    /// Create new [`PeerConnection`]. Provided  [`peer_events_sender`] will be
    /// used to emit any events from [`PeerEvent`].
    pub fn new(
        peer_id: Id,
        peer_events_sender: &UnboundedSender<PeerEvent>,
    ) -> Result<Self, WasmErr> {
        let peer = Rc::new(RtcPeerConnection::new()?);
        let connections = Rc::new(RefCell::new(MediaConnections::new()));

        // Bind to `icecandidate` event.
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
                            candidate: candidate.candidate(),
                            sdp_m_line_index: candidate.sdp_m_line_index(),
                            sdp_mid: candidate.sdp_mid(),
                        },
                    );
                }
            },
        )?;

        // Bind to `track` event.
        let sender = peer_events_sender.clone();
        let connections_rc = Rc::clone(&connections);
        let on_track = EventListener::new_mut(
            Rc::clone(&peer),
            "track",
            move |track_event: RtcTrackEvent| {


//                let mut connections = connections_rc.borrow_mut();
//
//                let transceiver = track_event.transceiver();
//                let track = track_event.track();
//
//                if let Some(receiver) =
//                    connections.add_remote_track(transceiver, track)
//                {
//                    let sender_id = receiver.sender_id;
//
//                    // gather all tracks from new track sender, break if still
//                    // waiting for some tracks
//                    let mut tracks: Vec<&StreamTrack> = Vec::new();
//                    for receiver in connections.get_by_sender(sender_id) {
//                        match receiver.track {
//                            None => return,
//                            Some(ref track) => tracks.push(track),
//                        }
//                    }
//
//                    // build and set MediaStream in receivers
//                    let stream = MediaStream::from_tracks(tracks);
//                    for receiver in connections.get_by_sender(sender_id) {
//                        let stream = Rc::clone(&stream);
//                        receiver.stream.replace(stream);
//                    }
//
//                    let _ = sender.unbounded_send(PeerEvent::NewRemoteStream {
//                        peer_id,
//                        sender_id,
//                        remote_stream: Rc::clone(&stream),
//                    });
//                } else {
//                    // TODO: means that this peer is out of sync, should be
//                    //       handled somehow (propagated to medea to init peer
//                    //       recreation?)
//                }


            },
        )?;

        Ok(Self {
            peer,
            _on_ice_candidate: on_ice_candidate,
            _on_track: on_track,
            media_connections: connections,
        })
    }

    pub fn get_mids(&self) -> Result<Option<HashMap<u64, String>>, WasmErr> {
        if self.media_connections.borrow().senders.is_empty() {
            return Ok(None);
        }

        let mut mids = HashMap::new();
        for (track, sender) in &self.media_connections.borrow().senders {
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

    /// Add ice candidate from remote peer to this peer.
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
            self.peer.add_ice_candidate_with_opt_rtc_ice_candidate_init(
                Some(cand_init).as_ref(),
            ),
        )
        .map(|_| ())
        .map_err(Into::into)
    }

    /// Update peers tracks.
    ///
    /// Synchronize provided tracks with this peer [`media_connections`]. Each
    /// [`Direction::Send`] track have corresponding [`Sender`], and each
    /// [`Direction::Recv`] track has [`Receiver`].
    pub fn update_tracks(
        &self,
        tracks: Vec<Track>,
        media_manager: &Rc<MediaManager>,
    ) -> impl Future<Item = (), Error = ()> {
        // insert peer transceivers
        for track in tracks {
            match track.direction {
                Direction::Send { .. } => {
                    self.media_connections.borrow_mut().senders.insert(
                        track.id,
                        Sender::new(&self.peer, track.id, track.media_type),
                    );
                }
                Direction::Recv { sender, mid } => {
                    self.media_connections
                        .borrow_mut()
                        .receivers
                        .insert(track.id, Receiver::new(sender, mid));
                }
            }
        }

        // if senders not empty, then get media from media manager and insert
        // tracks into transceivers
        if self.media_connections.borrow().senders.is_empty() {
            future::Either::A(future::ok(()))
        } else {
            let mut media_request = StreamRequest::new();
            for (track_id, sender) in &self.media_connections.borrow().senders {
                media_request
                    .add_track_request(*track_id, sender.media_type.clone());
            }

            let media_manager = Rc::clone(media_manager);
            let connections = Rc::clone(&self.media_connections);
            let get_media = media_manager
                .get_stream(media_request)
                .and_then(move |stream: Rc<MediaStream>| {

                    let mut promises = Vec::new();

                    for sender in connections.borrow().senders.values() {
                        let sender: &Sender = sender;

                        if let Some(track) = stream.get_track_by_id(sender.track_id) {
                            promises.push(sender.transceiver.sender().replace_track(Some(track.track())));
                        } else {

                        }
                    }

//                        let rtc_sender: RtcRtpSender = sender.transceiver.sender();
//                        let replace = match sender.media_type {
//                            MediaType::Audio(_) => rtc_sender
//                                .replace_track(stream.get_audio_track()),
//
//                            MediaType::Video(_) =>
//                                rtc_sender
//                                    .replace_track(stream.get_video_track()),
//                        };
//
//                        promises.push(
//                            JsFuture::from(replace).map_err(WasmErr::from),
//                        );
//                    join_all(promises).map_err(|err:
//                                                WasmErr| err.log_err());
                    Ok(())
                })
                .map(|_| ());
            future::Either::B(get_media)
        }
    }
}

struct MediaConnections {
    senders: HashMap<u64, Sender>,
    receivers: HashMap<u64, Receiver>,
}

impl MediaConnections {
    pub fn new() -> Self {
        Self {
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }
    }

    /// Find associated [`Receiver`] by transceiver's mid and update it with
    /// [`StreamTrack`] and [`RtcRtpTransceiver`] and return found
    /// [`Receiver`].
    pub fn add_remote_track(
        &mut self,
        transceiver: RtcRtpTransceiver,
        track: MediaTrack,
    ) -> Option<&Receiver> {
        // should be safe to unwrap
        let mid = transceiver.mid().unwrap();

        for receiver in &mut self.receivers.values_mut() {
            if let Some(recv_mid) = &receiver.mid {
                if recv_mid == &mid {
                    receiver.transceiver.replace(transceiver);
                    receiver.track.replace(track);
                    return Some(receiver);
                }
            }
        }

        None
    }

    /// Returns [`Receiver`]s that share provided sender id.
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

/// Local track representation, that is being sent to some remote peer.
pub struct Sender {
    track_id: u64,
    transceiver: RtcRtpTransceiver,
    media_type: MediaType,
}

impl Sender {
    fn new(peer: &Rc<RtcPeerConnection>, track_id:u64, media_type: MediaType) -> Self {
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
            track_id,
            transceiver,
            media_type,
        }
    }
}

/// Remote track representation that is being received from some remote peer.
pub struct Receiver {
    transceiver: Option<RtcRtpTransceiver>,
    sender_id: u64,
    mid: Option<String>,
    track: Option<MediaTrack>,
}

impl Receiver {
    fn new(sender_id: u64, mid: Option<String>) -> Self {
        Self {
            transceiver: None,
            sender_id,
            mid,
            track: None,
        }
    }
}

impl Drop for PeerConnection {
    fn drop(&mut self) {
        self.peer.close()
    }
}
