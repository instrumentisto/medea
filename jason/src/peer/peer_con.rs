use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::{
    future::{self, join_all},
    sync::mpsc::UnboundedSender,
    Future,
};
use medea_client_api_proto::{Direction, IceServer, MediaType, Track};
use medea_macro::dispatchable;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    RtcConfiguration, RtcIceCandidateInit, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcRtpTransceiver, RtcRtpTransceiverDirection,
    RtcRtpTransceiverInit, RtcSdpType, RtcSessionDescription,
    RtcSessionDescriptionInit, RtcTrackEvent,
};

use crate::{
    media::{MediaManager, MediaStream, MediaTrack, StreamRequest, TrackId},
    peer::ice_server::RtcIceServers,
    utils::{EventListener, WasmErr},
};

pub type Id = u64;

/// [`RTCSdpType`][1] adapter. `pranswer` and `rollback` SDPs aren't used.
///
/// [1]: https://www.w3.org/TR/webrtc/#rtcsdptype
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
        remote_stream: MediaStream,
    },
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection {
    /// Underlying [`RtcPeerConnection`].
    peer: Rc<RtcPeerConnection>,

    /// [`RtcPeerConnection`][1]'s [`on_ice_candidate`][2] callback. Which
    /// fires when [`RtcPeerConnection`][1] discovers new ice candidate.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-onicecandidate
    _on_ice_candidate:
        EventListener<RtcPeerConnection, RtcPeerConnectionIceEvent>,

    /// [`RtcPeerConnection`][1]'s [`on_track`][2] callback. Which fires when
    /// [`RtcPeerConnection`][1] receives new [`StreamTrack`] from remote
    /// peer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcpeerconnection-ontrack
    _on_track: EventListener<RtcPeerConnection, RtcTrackEvent>,

    /// [`Sender`]s and [`Receivers`] of this [`PeerConnection`].
    media_connections: Rc<RefCell<MediaConnections>>,
}

impl PeerConnection {
    /// Create new [`PeerConnection`]. Provided  `peer_events_sender` will be
    /// used to emit any events from [`PeerEvent`], provided `ice_servers` will
    /// be used by created [`PeerConenction`].
    pub fn new(
        peer_id: Id,
        peer_events_sender: &UnboundedSender<PeerEvent>,
        ice_servers: Vec<IceServer>,
    ) -> Result<Self, WasmErr> {
        let mut peer_conf = RtcConfiguration::new();

        peer_conf.ice_servers(&RtcIceServers::from(ice_servers));

        let peer =
            Rc::new(RtcPeerConnection::new_with_configuration(&peer_conf)?);
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
                let mut connections = connections_rc.borrow_mut();
                let transceiver = track_event.transceiver();
                let track = track_event.track();

                if let Some(receiver) =
                    connections.add_remote_track(transceiver, track)
                {
                    let sender_id = receiver.sender_id;

                    // gather all tracks from new track sender, break if still
                    // waiting for some tracks
                    let mut tracks: Vec<Rc<MediaTrack>> = Vec::new();
                    for receiver in connections.get_by_sender(sender_id) {
                        match receiver.track {
                            None => return,
                            Some(ref track) => tracks.push(Rc::clone(track)),
                        }
                    }

                    // got all tracks from this sender, so emit
                    // PeerEvent::NewRemoteStream
                    let _ = sender.unbounded_send(PeerEvent::NewRemoteStream {
                        peer_id,
                        sender_id,
                        remote_stream: MediaStream::from_tracks(tracks),
                    });
                } else {
                    // TODO: means that this peer is out of sync, should be
                    //       handled somehow (propagated to medea to init peer
                    //       recreation?)
                }
            },
        )?;

        Ok(Self {
            peer,
            _on_ice_candidate: on_ice_candidate,
            _on_track: on_track,
            media_connections: connections,
        })
    }

    /// Track id to mid relations of all send tracks of this
    /// [`PeerConnection`]. mid is id of [`m= section`][1]. mids are received
    /// directly from registered [`RTCRtpTransceiver`][2]s, and are being
    /// allocated on local sdp update (i.e. after `create_and_set_offer` call).
    /// Errors if finds sending transceiver without mid.
    ///
    /// [1]: https://tools.ietf.org/html/rfc4566#section-5.14
    /// [2]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    pub fn get_send_mids(
        &self,
    ) -> Result<Option<HashMap<u64, String>>, WasmErr> {
        if self.media_connections.borrow().senders.is_empty() {
            return Ok(None);
        }

        let mut mids = HashMap::new();
        for (track_id, sender) in &self.media_connections.borrow().senders {
            mids.insert(
                *track_id,
                sender.transceiver.mid().ok_or_else(|| {
                    WasmErr::build_from_str("Peer has senders without mid")
                })?,
            );
        }

        Ok(Some(mids))
    }

    /// Obtain SDP Offer from underlying [`RTCPeerConnection`][1] and set it as
    /// local description. Should be called after changing local tracks, but
    /// not all changes require renegotiation.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
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

    /// Obtain SDP Answer from underlying [`RTCPeerConnection`][1] and set it as
    /// local description. Should be called whenever remote description is
    /// changed.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
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

    /// Updates underlying [`RTCPeerConnection`][1] remote SDP.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
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
            self.peer.add_ice_candidate_with_opt_rtc_ice_candidate_init(
                Some(cand_init).as_ref(),
            ),
        )
        .map(|_| ())
        .map_err(Into::into)
    }

    // TODO: Doesnt really updates anything, but only generates new senders and
    //       receivers atm.
    fn update_track(&self, track: Track) {
        match track.direction {
            Direction::Send { .. } => {
                self.media_connections.borrow_mut().senders.insert(
                    track.id,
                    Sender::new(track.id, track.media_type, &self.peer),
                );
            }
            Direction::Recv { sender, mid } => {
                self.media_connections.borrow_mut().receivers.insert(
                    track.id,
                    Receiver::new(track.id, track.media_type, sender, mid),
                );
            }
        }
    }

    /// Update this peer [`Receiver`]s mids.
    pub fn set_recv_mids(&self, mids: HashMap<u64, String>) {
        for (track_id, mid) in mids {
            if let Some(receiver) = self
                .media_connections
                .borrow_mut()
                .receivers
                .get_mut(&track_id)
            {
                receiver.mid.replace(mid);
            }
        }
    }

    /// Update peers tracks.
    ///
    /// Synchronize provided tracks with this [`PeerConnection`] [`Sender`]s and
    /// [`Receiver`]s. Will proc local media request if required.
    pub fn update_tracks(
        &self,
        tracks: Vec<Track>,
        media_manager: &Rc<MediaManager>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        for track in tracks {
            self.update_track(track);
        }

        if let Some(media_request) =
            self.media_connections.borrow().get_request()
        {
            let media_manager = Rc::clone(media_manager);
            let connections = Rc::clone(&self.media_connections);
            let get_media = media_manager.get_stream(media_request).and_then(
                move |stream| {
                    connections.borrow_mut().insert_local_stream(&stream)
                },
            );
            future::Either::B(get_media)
        } else {
            future::Either::A(future::ok(()))
        }
    }
}

/// Stores [`Peer`]s [`Sender`]s and [`Receiver`]s.
struct MediaConnections {
    need_local_stream: bool,
    senders: HashMap<TrackId, Sender>,
    receivers: HashMap<TrackId, Receiver>,
}

impl MediaConnections {
    pub fn new() -> Self {
        Self {
            need_local_stream: false,
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }
    }

    /// Check if [`Sender`]s require new [`MediaStream`].
    fn get_request(&self) -> Option<StreamRequest> {
        if self.need_local_stream {
            let mut media_request = StreamRequest::default();
            for (track_id, sender) in &self.senders {
                media_request.add_track_request(*track_id, sender.caps.clone());
            }
            Some(media_request)
        } else {
            None
        }
    }

    /// Inserts tracks from provided [`MediaStream`] into stored [`Sender`]s
    /// based on track ids. Stream must have all required tracks.
    pub fn insert_local_stream(
        &mut self,
        stream: &Rc<MediaStream>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        // validate that provided stream have all tracks that we need
        for sender in self.senders.values() {
            if !stream.has_track(sender.track_id) {
                return future::Either::A(future::err(
                    WasmErr::build_from_str(
                        "Stream does not have all necessary tracks",
                    ),
                ));
            }
        }

        let mut promises = Vec::new();
        for sender in self.senders.values() {
            let sender: &Sender = sender;

            if let Some(track) = stream.get_track_by_id(sender.track_id) {
                promises.push(
                    JsFuture::from(
                        sender
                            .transceiver
                            .sender()
                            .replace_track(Some(track.track())),
                    )
                    .map_err(WasmErr::from),
                );
            }
        }

        future::Either::B(join_all(promises).map(|_| ()))
    }

    /// Find associated [`Receiver`] by transceiver's mid and update it with
    /// [`StreamTrack`] and [`RtcRtpTransceiver`][1] and return found
    /// [`Receiver`].
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    pub fn add_remote_track(
        &mut self,
        transceiver: RtcRtpTransceiver,
        track: web_sys::MediaStreamTrack,
    ) -> Option<&Receiver> {
        // should be safe to unwrap
        let mid = transceiver.mid().unwrap();

        for receiver in &mut self.receivers.values_mut() {
            if let Some(recv_mid) = &receiver.mid {
                if recv_mid == &mid {
                    let track = MediaTrack::new(
                        receiver.track_id,
                        track,
                        receiver.caps.clone(),
                    );

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
    track_id: TrackId,
    transceiver: RtcRtpTransceiver,
    caps: MediaType,
}

impl Sender {
    fn new(
        track_id: TrackId,
        caps: MediaType,
        peer: &Rc<RtcPeerConnection>,
    ) -> Self {
        let transceiver = match caps {
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
            caps,
        }
    }
}

/// Remote track representation that is being received from some remote peer.
/// Basically, it can have two states: waiting and receiving. When track arrives
/// we can save related [`RtcRtpTransceiver`][1] and actual [`MediaTrack`].
///
/// [1]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
pub struct Receiver {
    track_id: TrackId,
    caps: MediaType,
    sender_id: u64,
    transceiver: Option<RtcRtpTransceiver>,
    mid: Option<String>,
    track: Option<Rc<MediaTrack>>,
}

impl Receiver {
    fn new(
        track_id: TrackId,
        caps: MediaType,
        sender_id: u64,
        mid: Option<String>,
    ) -> Self {
        Self {
            track_id,
            caps,
            sender_id,
            transceiver: None,
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
