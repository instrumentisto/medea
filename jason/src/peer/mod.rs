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

use crate::{
    media::{MediaManager, MediaStream},
    peer::{
        media_connections::MediaConnections,
        peer_con::{RtcPeerConnection, SdpType},
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

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection {
    /// Underlying [`RtcPeerConnection`].
    peer: std::rc::Rc<crate::peer::peer_con::RtcPeerConnection>,

    /// [`Sender`]s and [`Receivers`] of this [`PeerConnection`].
    media_connections: Rc<MediaConnections>,

    media_manager: Rc<MediaManager>,
}

impl PeerConnection {
    /// Create new [`PeerConnection`]. Provided  `peer_events_sender` will be
    /// used to emit any events from [`PeerEvent`], provided `ice_servers` will
    /// be used by created [`PeerConenction`].
    pub fn new(
        peer_id: Id,
        peer_events_sender: &UnboundedSender<PeerEvent>,
        ice_servers: Vec<IceServer>,
        media_manager: Rc<MediaManager>,
    ) -> Result<Self, WasmErr> {
        let peer = Rc::new(RtcPeerConnection::new(ice_servers)?);
        let connections = Rc::new(MediaConnections::new(Rc::clone(&peer)));

        // Bind to `icecandidate` event.
        let sender = peer_events_sender.clone();
        peer.on_ice_candidate(move |candidate| {
            let _ = sender.unbounded_send(PeerEvent::IceCandidateDiscovered {
                peer_id,
                candidate: candidate.candidate,
                sdp_m_line_index: candidate.sdp_m_line_index,
                sdp_mid: candidate.sdp_mid,
            });
        })?;

        // Bind to `track` event.
        let sender = peer_events_sender.clone();
        let connections_rc = Rc::clone(&connections);
        peer.on_track(move |track_event| {
            let connections = &connections_rc;
            let transceiver = track_event.transceiver();
            let track = track_event.track();

            if let Some(sender_id) =
                connections.add_remote_track(transceiver, track)
            {
                if let Some(tracks) =
                    connections.get_tracks_by_sender(sender_id)
                {
                    // got all tracks from this sender, so emit
                    // PeerEvent::NewRemoteStream
                    let _ = sender.unbounded_send(PeerEvent::NewRemoteStream {
                        peer_id,
                        sender_id,
                        remote_stream: MediaStream::from_tracks(tracks),
                    });
                };
            } else {
                // TODO: means that this peer is out of sync, should be
                //       handled somehow (propagated to medea to init peer
                //       recreation?)
            }
        })?;

        Ok(Self {
            peer,
            media_connections: connections,
            media_manager,
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
    pub fn get_mids(&self) -> Result<HashMap<u64, String>, WasmErr> {
        self.media_connections.get_mids()
    }

    pub fn create_and_set_offer(
        &self,
        tracks: Vec<Track>,
    ) -> impl Future<Item = String, Error = WasmErr> {
        if let Err(err) = self.media_connections.update_tracks(tracks) {
            return future::Either::A(future::err(err));
        }

        let peer: std::rc::Rc<crate::peer::peer_con::RtcPeerConnection> =
            Rc::clone(&self.peer);
        future::Either::B(
            match self.media_connections.get_request() {
                None => future::Either::A(future::ok::<_, WasmErr>(())),
                Some(request) => {
                    let media_connections = Rc::clone(&self.media_connections);
                    future::Either::B(
                        self.media_manager.get_stream(request).and_then(
                            move |stream| {
                                media_connections.insert_local_stream(&stream)
                            },
                        ),
                    )
                }
            }
            .and_then(move |_| peer.create_and_set_offer()),
        )
    }

    pub fn create_and_set_answer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        self.peer.create_and_set_answer()
    }

    /// Updates underlying [`RTCPeerConnection`][1] remote SDP.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub fn set_remote_answer(
        &self,
        answer: String,
    ) -> impl Future<Item = (), Error = WasmErr> {
        self.peer.set_remote_description(SdpType::Answer(answer))
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
        self.media_connections.update_tracks(recv).unwrap();

        let media_connections = Rc::clone(&self.media_connections);
        let media_manager = Rc::clone(&self.media_manager);
        self.peer
            .set_remote_description(SdpType::Offer(offer))
            .and_then(move |_| {
                media_connections
                    .update_tracks(send)
                    .map(|_| media_connections.get_request())
                    .map(|req| (req, media_connections))
            })
            .and_then(move |(request, cons)| match request {
                None => future::Either::A(future::ok::<_, WasmErr>(())),
                Some(request) => future::Either::B(
                    media_manager
                        .get_stream(request)
                        .and_then(move |s| cons.insert_local_stream(&s)),
                ),
            })
    }

    pub fn add_ice_candidate(
        &self,
        candidate: &str,
        sdp_m_line_index: Option<u16>,
        sdp_mid: &Option<String>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        self.peer
            .add_ice_candidate(candidate, sdp_m_line_index, sdp_mid)
    }
}
