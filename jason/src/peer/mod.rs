//! Adapters to [`RTCPeerConnection`][1] and related objects.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

mod ice_server;
mod media_connections;
mod peer_con;
mod repo;

use std::{collections::HashMap, rc::Rc};

use futures::{future, sync::mpsc::UnboundedSender, Future};
use medea_client_api_proto::{Direction, IceServer, Track};
use medea_macro::dispatchable;
use web_sys::RtcTrackEvent;

use crate::{
    media::{MediaManager, MediaStream},
    peer::{
        media_connections::MediaConnections,
        peer_con::{IceCandidate, RtcPeerConnection, SdpType},
    },
    utils::WasmErr,
};

#[doc(inline)]
pub use self::{repo::PeerRepository, Id as PeerId};

pub type Id = u64;

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

struct InnerPeerConnection {
    id: Id,

    /// Underlying [`RtcPeerConnection`].
    peer: Rc<RtcPeerConnection>,

    /// [`Sender`]s and [`Receivers`] of this [`PeerConnection`].
    media_connections: MediaConnections,

    /// [`MediaManager`] that will be used to acquire local [`MediaStream`]s.
    media_manager: Rc<MediaManager>,

    /// [`PeerEvent`]s tx.
    peer_events_sender: UnboundedSender<PeerEvent>,
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection(Rc<InnerPeerConnection>);

impl PeerConnection {
    /// Create new [`PeerConnection`]. Provided  `peer_events_sender` will be
    /// used to emit any events from [`PeerEvent`], provided `ice_servers` will
    /// be used by created [`PeerConenction`].
    pub fn new(
        id: Id,
        peer_events_sender: UnboundedSender<PeerEvent>,
        ice_servers: Vec<IceServer>,
        media_manager: Rc<MediaManager>,
    ) -> Result<Self, WasmErr> {
        let peer = Rc::new(RtcPeerConnection::new(ice_servers)?);
        let media_connections = MediaConnections::new(Rc::clone(&peer));
        let inner = Rc::new(InnerPeerConnection {
            id,
            peer,
            media_connections,
            media_manager,
            peer_events_sender,
        });

        // Bind to `icecandidate` event.
        let inner_rc = Rc::clone(&inner);
        inner.peer.on_ice_candidate(Some(move |candidate| {
            Self::on_ice_candidate(&inner_rc, candidate);
        }))?;

        // Bind to `track` event.
        let inner_rc = Rc::clone(&inner);
        inner.peer.on_track(Some(move |track_event| {
            Self::on_track(&inner_rc, &track_event);
        }))?;

        Ok(Self(inner))
    }

    /// Handle `icecandidate` event from underlying peer emitting
    /// [`PeerEvent::IceCandidateDiscovered`] event into this peers
    /// `peer_events_sender`.
    fn on_ice_candidate(inner: &InnerPeerConnection, candidate: IceCandidate) {
        let _ = inner.peer_events_sender.unbounded_send(
            PeerEvent::IceCandidateDiscovered {
                peer_id: inner.id,
                candidate: candidate.candidate,
                sdp_m_line_index: candidate.sdp_m_line_index,
                sdp_mid: candidate.sdp_mid,
            },
        );
    }

    /// Handle `track` event from underlying peer emitting
    /// [`PeerEvent::NewRemoteStream`] event into this peers
    /// `peer_events_sender` if all tracks from this sender arrived.
    fn on_track(inner: &InnerPeerConnection, track_event: &RtcTrackEvent) {
        let transceiver = track_event.transceiver();
        let track = track_event.track();

        if let Some(sender_id) =
            inner.media_connections.add_remote_track(transceiver, track)
        {
            if let Some(tracks) =
                inner.media_connections.get_tracks_by_sender(sender_id)
            {
                // got all tracks from this sender, so emit
                // PeerEvent::NewRemoteStream
                let _ = inner.peer_events_sender.unbounded_send(
                    PeerEvent::NewRemoteStream {
                        peer_id: inner.id,
                        sender_id,
                        remote_stream: MediaStream::from_tracks(tracks),
                    },
                );
            };
        } else {
            // TODO: means that this peer is out of sync, should be
            //       handled somehow (propagated to medea to init peer
            //       recreation?)
        }
    }

    /// Track id to mid relations of all send tracks of this
    /// [`PeerConnection`]. mid is id of [`m= section`][1]. mids are received
    /// directly from registered [`RTCRtpTransceiver`][2]s, and are being
    /// allocated on local sdp update (i.e. after `create_and_set_offer` call).
    /// Errors if finds sending transceiver without mid.
    ///
    /// [1]: https://tools.ietf.org/html/rfc4566#section-5.14
    /// [2]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    pub fn get_mids(&self) -> Result<HashMap<u64, String>, WasmErr> {
        self.0.media_connections.get_mids()
    }

    /// Sync provided tracks creating send and recv transceivers, requesting
    /// local stream if required. Get, set and return sdp offer.
    pub fn get_offer(
        &self,
        tracks: Vec<Track>,
    ) -> impl Future<Item = String, Error = WasmErr> {
        if let Err(err) = self.0.media_connections.update_tracks(tracks) {
            return future::Either::A(future::err(err));
        }

        let inner: Rc<InnerPeerConnection> = Rc::clone(&self.0);
        let peer = Rc::clone(&self.0.peer);
        future::Either::B(
            match self.0.media_connections.get_request() {
                None => future::Either::A(future::ok::<_, WasmErr>(())),
                Some(request) => future::Either::B(
                    inner.media_manager.get_stream(request).and_then(
                        move |stream| {
                            inner.media_connections.insert_local_stream(&stream)
                        },
                    ),
                ),
            }
            .and_then(move |_| peer.create_and_set_offer()),
        )
    }

    pub fn create_and_set_answer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        self.0.peer.create_and_set_answer()
    }

    /// Updates underlying [`RTCPeerConnection`][1] remote SDP.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub fn set_remote_answer(
        &self,
        answer: String,
    ) -> impl Future<Item = (), Error = WasmErr> {
        self.0.peer.set_remote_description(SdpType::Answer(answer))
    }

    /// Updates [`PeerConnection`] tracks and sets remote offer.
    /// `set_remote_description` will create all transceivers and fire all
    /// `on_track` events, so it updates [`Receiver`]s before
    /// `set_remote_description` and update [`Sender`]s after.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub fn process_offer(
        &self,
        offer: String,
        tracks: Vec<Track>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        // TODO: use drain_filter when its stable
        let (recv, send): (Vec<_>, Vec<_>) =
            tracks.into_iter().partition(|track| match track.direction {
                Direction::Send { .. } => false,
                Direction::Recv { .. } => true,
            });

        // update receivers
        self.0.media_connections.update_tracks(recv).unwrap();

        let inner: Rc<InnerPeerConnection> = Rc::clone(&self.0);
        self.0
            .peer
            .set_remote_description(SdpType::Offer(offer))
            .and_then(move |_| {
                inner
                    .media_connections
                    .update_tracks(send)
                    .map(|_| inner.media_connections.get_request())
                    .map(|req| (req, inner))
            })
            .and_then(move |(request, inner)| match request {
                None => future::Either::A(future::ok::<_, WasmErr>(())),
                Some(request) => future::Either::B(
                    inner.media_manager.get_stream(request).and_then(
                        move |s| {
                            inner.media_connections.insert_local_stream(&s)
                        },
                    ),
                ),
            })
    }

    pub fn add_ice_candidate(
        &self,
        candidate: &str,
        sdp_m_line_index: Option<u16>,
        sdp_mid: &Option<String>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        self.0
            .peer
            .add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
    }
}

impl Drop for PeerConnection {
    fn drop(&mut self) {
        let _ = self.0.peer.on_track::<Box<FnMut(RtcTrackEvent)>>(None);
        let _ = self.0.peer.on_ice_candidate::<Box<FnMut(IceCandidate)>>(None);
    }
}
