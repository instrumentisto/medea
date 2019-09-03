//! Adapters to [`RTCPeerConnection`][1] and related objects.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

mod conn;
mod ice_server;
mod media;
mod repo;

use std::{cell::RefCell, collections::HashMap, ops::Deref, rc::Rc};

use futures::{future, sync::mpsc::UnboundedSender, Future};
use medea_client_api_proto::{Direction, IceServer, Track};
use medea_macro::dispatchable;
use web_sys::RtcTrackEvent;

use crate::{
    media::{MediaManager, MediaStream},
    utils::WasmErr,
};

use self::{
    conn::{IceCandidate, RtcPeerConnection, SdpType},
    media::MediaConnections,
};

#[doc(inline)]
pub use self::{repo::PeerRepository, Id as PeerId};
use futures::future::Either;

pub type Id = u64;

#[dispatchable]
#[allow(clippy::module_name_repetitions)]
/// Events emitted from [`RtcPeerConnection`].
pub enum PeerEvent {
    /// [`RtcPeerConnection`] discovered new ice candidate.
    IceCandidateDiscovered {
        peer_id: Id,
        candidate: String,
        sdp_m_line_index: Option<u16>,
        sdp_mid: Option<String>,
    },

    /// [`RtcPeerConnection`] received new stream from remote sender.
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

    /// [`Sender`]s and [`Receivers`] of this [`RtcPeerConnection`].
    media_connections: MediaConnections,

    /// [`MediaManager`] that will be used to acquire local [`MediaStream`]s.
    media_manager: Rc<MediaManager>,

    /// [`PeerEvent`]s tx.
    peer_events_sender: UnboundedSender<PeerEvent>,

    has_remote_description: bool,

    ice_candidates: Vec<(String, Option<u16>, Option<String>)>,
}

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection(Rc<RefCell<InnerPeerConnection>>);

impl PeerConnection {
    /// Create new [`RtcPeerConnection`]. Provided `peer_events_sender` will be
    /// used to emit [`PeerEvent`]s from this peer , provided `ice_servers` will
    /// be used by created [`RtcPeerConnection`].
    pub fn new<I: IntoIterator<Item = IceServer>>(
        id: Id,
        peer_events_sender: UnboundedSender<PeerEvent>,
        ice_servers: I,
        media_manager: Rc<MediaManager>,
    ) -> Result<Self, WasmErr> {
        let peer = Rc::new(RtcPeerConnection::new(ice_servers)?);
        let media_connections = MediaConnections::new(Rc::clone(&peer));
        let inner = Rc::new(RefCell::new(InnerPeerConnection {
            id,
            peer,
            media_connections,
            media_manager,
            peer_events_sender,
            has_remote_description: false,
            ice_candidates: vec![],
        }));

        // Bind to `icecandidate` event.
        let inner_rc = Rc::clone(&inner);
        inner
            .borrow()
            .peer
            .on_ice_candidate(Some(move |candidate| {
                Self::on_ice_candidate(inner_rc.borrow().deref(), candidate);
            }))?;

        // Bind to `track` event.
        let inner_rc = Rc::clone(&inner);
        inner.borrow().peer.on_track(Some(move |track_event| {
            Self::on_track(inner_rc.borrow().deref(), &track_event);
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

    /// Handle `track` event from underlying peer adding new track to
    /// `media_connections` and emitting [`PeerEvent::NewRemoteStream`]
    /// event into this peers `peer_events_sender` if all tracks from this
    /// sender has arrived.
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
    /// [`RtcPeerConnection`]. mid is id of [`m= section`][1]. mids are received
    /// directly from registered [`RTCRtpTransceiver`][2]s, and are being
    /// allocated on sdp update.
    /// Errors if finds transceiver without mid, so must be called after setting
    /// local description if offerrer, and remote if answerer.
    ///
    /// [1]: https://tools.ietf.org/html/rfc4566#section-5.14
    /// [2]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    pub fn get_mids(&self) -> Result<HashMap<u64, String>, WasmErr> {
        self.0.borrow().media_connections.get_mids()
    }

    /// Sync provided tracks creating all required `Sender`s and
    /// `Receiver`s, request local stream if required, get, set and return
    /// sdp offer.
    pub fn get_offer(
        &self,
        tracks: Vec<Track>,
    ) -> impl Future<Item = String, Error = WasmErr> {
        match self.0.borrow().media_connections.update_tracks(tracks) {
            Err(err) => future::Either::A(future::err(err)),
            Ok(request) => {
                let peer = Rc::clone(&self.0.borrow().peer);
                future::Either::B(
                    match request {
                        None => future::Either::A(future::ok::<_, WasmErr>(())),
                        Some(request) => {
                            let inner: Rc<RefCell<InnerPeerConnection>> =
                                Rc::clone(&self.0);
                            future::Either::B(
                                self.0
                                    .borrow()
                                    .media_manager
                                    .get_stream(request)
                                    .and_then(move |stream| {
                                        inner
                                            .borrow()
                                            .media_connections
                                            .insert_local_stream(&stream)
                                    }),
                            )
                        }
                    }
                    .and_then(move |_| peer.create_and_set_offer()),
                )
            }
        }
    }

    /// Creates an SDP answer to an offer received from a remote peer and sets
    /// it as local description. Must be called only if peer already has remote
    /// description.
    pub fn create_and_set_answer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        self.0.borrow().peer.create_and_set_answer()
    }

    /// Updates underlying [`RTCPeerConnection`][1] remote SDP.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub fn set_remote_answer(
        &self,
        answer: String,
    ) -> impl Future<Item = (), Error = WasmErr> {
        let inner = Rc::clone(&self.0);
        self.0
            .borrow()
            .peer
            .set_remote_description(SdpType::Answer(answer))
            .and_then(move |_| {
                inner.borrow_mut().has_remote_description = true;
                Ok(())
            })
    }

    /// Sync provided tracks creating all required `Sender`s and
    /// `Receiver`s, request local stream if required.
    /// `set_remote_description` will create all transceivers and fire all
    /// `on_track` events, so it updates `Receiver`s before
    /// `set_remote_description` and update `Sender`s after.
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
        self.0
            .borrow()
            .media_connections
            .update_tracks(recv)
            .unwrap();

        let inner: Rc<RefCell<InnerPeerConnection>> = Rc::clone(&self.0);
        self.0
            .borrow()
            .peer
            .set_remote_description(SdpType::Offer(offer))
            .and_then(move |_| {
                let request =
                    inner.borrow().media_connections.update_tracks(send)?;
                Ok((request, inner))
            })
            .and_then(|(request, inner)| match request {
                None => future::Either::A(future::ok::<_, WasmErr>(())),
                Some(request) => {
                    let media_manager =
                        Rc::clone(&inner.borrow().media_manager);
                    future::Either::B(
                        media_manager.get_stream(request).and_then(move |s| {
                            inner
                                .borrow()
                                .media_connections
                                .insert_local_stream(&s)
                        }),
                    )
                }
            })
    }

    /// Adds remote peers [ICE Candidate][1] to this peer.
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-2
    pub fn add_ice_candidate(
        &self,
        candidate: String,
        sdp_m_line_index: Option<u16>,
        sdp_mid: Option<String>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        let mut inner = self.0.borrow_mut();
        if inner.has_remote_description {
            let peer = Rc::clone(&inner.peer);
            let mut fut = inner.ice_candidates.drain(..).fold(
                vec![],
                move |mut acc, (candidate, sdp_m_line_index, sdp_mid)| {
                    acc.push(peer.add_ice_candidate(
                        &candidate,
                        sdp_m_line_index,
                        &sdp_mid,
                    ));
                    acc
                },
            );
            fut.push(inner.peer.add_ice_candidate(
                &candidate,
                sdp_m_line_index,
                &sdp_mid,
            ));
            Either::A(future::join_all(fut).map(|_| ()).map_err(|e| {
                e.log_err();
                e
            }))
        } else {
            inner
                .ice_candidates
                .push((candidate, sdp_m_line_index, sdp_mid));
            WasmErr::from("Not have remote desc. Candidate stored.").log_err();
            Either::B(future::ok(()))
        }
    }
}

impl Drop for PeerConnection {
    /// Drop `on_track` and `on_ice_candidate` callbacks to prevent leak.
    fn drop(&mut self) {
        let _ = self
            .0
            .borrow()
            .peer
            .on_track::<Box<dyn FnMut(RtcTrackEvent)>>(None);
        let _ = self
            .0
            .borrow()
            .peer
            .on_ice_candidate::<Box<dyn FnMut(IceCandidate)>>(None);
    }
}
