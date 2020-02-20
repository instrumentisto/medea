//! Adapters to [RTCPeerConnection][1] and related objects.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

mod conn;
mod ice_server;
mod media;
mod repo;
mod stream;
mod stream_request;
mod track;

use std::{cell::RefCell, collections::HashMap, convert::TryFrom, rc::Rc};

use derive_more::{Display, From};
use futures::{channel::mpsc, future};
use medea_client_api_proto::{
    self as proto, Direction, IceConnectionState, IceServer, PeerId as Id,
    PeerId, Track, TrackId,
};
use medea_macro::dispatchable;
use tracerr::Traced;
use web_sys::{
    MediaStream as SysMediaStream, RtcIceConnectionState, RtcTrackEvent,
};

use crate::{
    media::{MediaManager, MediaManagerError},
    utils::{console_error, JsCaused, JsError},
};

#[cfg(feature = "mockable")]
#[doc(inline)]
pub use self::repo::MockPeerRepository;
#[doc(inline)]
pub use self::repo::{PeerRepository, Repository};
pub use self::{
    conn::{
        IceCandidate, RTCPeerConnectionError, RtcPeerConnection, SdpType,
        TransceiverDirection, TransceiverKind,
    },
    media::{
        MediaConnections, MediaConnectionsError, MuteState,
        MuteStateTransition, Sender, StableMuteState,
    },
    stream::{MediaStream, MediaStreamHandle},
    stream_request::{SimpleStreamRequest, StreamRequest, StreamRequestError},
    track::MediaTrack,
};

/// Errors that may occur in [RTCPeerConnection][1].
///
/// [1]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface
#[derive(Debug, Display, From, JsCaused)]
pub enum PeerError {
    /// Errors that may occur in [`MediaConnections`] storage.
    #[display(fmt = "{}", _0)]
    MediaConnections(#[js(cause)] MediaConnectionsError),

    /// Errors that may occur in a [`MediaManager`].
    #[display(fmt = "{}", _0)]
    MediaManager(#[js(cause)] MediaManagerError),

    /// Errors that may occur during signaling between this and remote
    /// [RTCPeerConnection][1] and event handlers setting errors.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection.
    #[display(fmt = "{}", _0)]
    RtcPeerConnection(#[js(cause)] RTCPeerConnectionError),

    /// Errors that may occur when validating [`StreamRequest`] or
    /// parsing [`MediaStream`].
    #[display(fmt = "{}", _0)]
    StreamRequest(#[js(cause)] StreamRequestError),
}

type Result<T> = std::result::Result<T, Traced<PeerError>>;

#[dispatchable]
/// Events emitted from [`RtcPeerConnection`].
pub enum PeerEvent {
    /// [`RtcPeerConnection`] discovered new ICE candidate.
    ///
    /// Wrapper around [RTCPeerConnectionIceEvent][1].
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtcpeerconnectioniceevent
    IceCandidateDiscovered {
        /// ID of the [`PeerConnection`] that discovered new ICE candidate.
        peer_id: Id,

        /// [`candidate` field][2] of the discovered [RTCIceCandidate][1].
        ///
        /// [1]: https://w3.org/TR/webrtc/#dom-rtcicecandidate
        /// [2]: https://w3.org/TR/webrtc/#dom-rtcicecandidate-candidate
        candidate: String,

        /// [`sdpMLineIndex` field][2] of the discovered [RTCIceCandidate][1].
        ///
        /// [1]: https://w3.org/TR/webrtc/#dom-rtcicecandidate
        /// [2]: https://w3.org/TR/webrtc/#dom-rtcicecandidate-sdpmlineindex
        sdp_m_line_index: Option<u16>,

        /// [`sdpMid` field][2] of the discovered [RTCIceCandidate][1].
        ///
        /// [1]: https://w3.org/TR/webrtc/#dom-rtcicecandidate
        /// [2]: https://w3.org/TR/webrtc/#dom-rtcicecandidate-sdpmid
        sdp_mid: Option<String>,
    },

    /// [`RtcPeerConnection`] received new stream from remote sender.
    NewRemoteStream {
        /// ID of the [`PeerConnection`] that received new stream from remote
        /// sender.
        peer_id: Id,

        /// ID of the remote sender's [`PeerConnection`].
        sender_id: Id,

        /// Received [`MediaStream`].
        remote_stream: MediaStream,
    },

    /// [`RtcPeerConnection`] sent new local stream to remote members.
    NewLocalStream {
        /// ID of the [`PeerConnection`] that sent new local stream to remote
        /// members.
        peer_id: Id,

        /// Local [`MediaStream`] that is sent to remote members.
        local_stream: MediaStream,
    },

    /// [`RtcPeerConnection`]'s [ICE connection][1] state changed.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dfn-ice-connection-state
    IceConnectionStateChanged {
        /// ID of the [`PeerConnection`] that sends
        /// [`iceconnectionstatechange`][1] event.
        ///
        /// [1]: https://w3.org/TR/webrtc/#event-iceconnectionstatechange
        peer_id: Id,

        /// New [`IceConnectionState`].
        ice_connection_state: IceConnectionState,
    },
}

/// High-level wrapper around [`RtcPeerConnection`].
pub struct PeerConnection {
    /// Unique ID of [`PeerConnection`].
    id: Id,

    /// Underlying [`RtcPeerConnection`].
    peer: Rc<RtcPeerConnection>,

    /// [`Sender`]s and [`Receiver`]s of this [`RtcPeerConnection`].
    ///
    /// [`Receiver`]: self::media::Receiver
    media_connections: Rc<MediaConnections>,

    /// [`MediaManager`] that will be used to acquire local [`MediaStream`]s.
    media_manager: Rc<MediaManager>,

    /// [`PeerEvent`]s tx.
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,

    /// Indicates if underlying [`RtcPeerConnection`] has remote description.
    has_remote_description: RefCell<bool>,

    /// Stores [`IceCandidate`]s received before remote description for
    /// underlying [`RtcPeerConnection`].
    ice_candidates_buffer: RefCell<Vec<IceCandidate>>,
}

impl PeerConnection {
    /// Creates new [`PeerConnection`].
    ///
    /// Provided `peer_events_sender` will be used to emit [`PeerEvent`]s from
    /// this peer.
    ///
    /// Provided `ice_servers` will be used by created [`RtcPeerConnection`].
    ///
    /// # Errors
    ///
    /// Errors with [`PeerError::RtcPeerConnection`] if [`RtcPeerConnection`]
    /// creating fails.
    ///
    /// Errors with [`PeerError::RtcPeerConnection`] if some callback of
    /// [`RtcPeerConnection`] can't be set.
    pub fn new<I: IntoIterator<Item = IceServer>>(
        id: Id,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        ice_servers: I,
        media_manager: Rc<MediaManager>,
        is_force_relayed: bool,
    ) -> Result<Self> {
        let peer = Rc::new(
            RtcPeerConnection::new(ice_servers, is_force_relayed)
                .map_err(tracerr::map_from_and_wrap!())?,
        );
        let media_connections =
            Rc::new(MediaConnections::new(Rc::clone(&peer)));

        let peer = Self {
            id,
            peer,
            media_connections,
            media_manager,
            peer_events_sender,
            has_remote_description: RefCell::new(false),
            ice_candidates_buffer: RefCell::new(vec![]),
        };

        // Bind to `icecandidate` event.
        let id = peer.id;
        let sender = peer.peer_events_sender.clone();
        peer.peer
            .on_ice_candidate(Some(move |candidate| {
                Self::on_ice_candidate(id, &sender, candidate);
            }))
            .map_err(tracerr::map_from_and_wrap!())?;

        // Bind to `iceconnectionstatechange` event.
        let id = peer.id;
        let sender = peer.peer_events_sender.clone();
        peer.peer
            .on_ice_connection_state_change(Some(move |ice_connection_state| {
                Self::on_ice_connection_state_changed(
                    id,
                    &sender,
                    ice_connection_state,
                );
            }))
            .map_err(tracerr::map_from_and_wrap!())?;

        // Bind to `track` event.
        let id = peer.id;
        let media_connections = Rc::clone(&peer.media_connections);
        let sender = peer.peer_events_sender.clone();
        peer.peer
            .on_track(Some(move |track_event| {
                Self::on_track(id, &media_connections, &sender, &track_event);
            }))
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(peer)
    }

    /// Returns `true` if all [`MediaTrack`]s of this [`PeerConnection`] is in
    /// the provided `mute_state`.
    #[inline]
    pub fn is_all_senders_in_mute_state(
        &self,
        kind: TransceiverKind,
        mute_state: StableMuteState,
    ) -> bool {
        self.media_connections
            .is_all_senders_in_mute_state(kind, mute_state)
    }

    /// Returns [`PeerId`] of this [`PeerConnection`].
    #[inline]
    pub fn id(&self) -> PeerId {
        self.id
    }

    /// Updates [`Sender`]s of this [`PeerConnection`] with
    /// [`proto::TrackPatch`].
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::InvalidTrackPatch`] if
    /// [`MediaTrack`] with ID from [`proto::TrackPatch`] doesn't exist.
    pub fn update_senders(&self, tracks: Vec<proto::TrackPatch>) -> Result<()> {
        Ok(self
            .media_connections
            .update_senders(tracks)
            .map_err(tracerr::map_from_and_wrap!())?)
    }

    /// Returns inner [`IceCandidate`]'s buffer len. Used in tests.
    pub fn candidates_buffer_len(&self) -> usize {
        self.ice_candidates_buffer.borrow().len()
    }

    /// Handle `icecandidate` event from underlying peer emitting
    /// [`PeerEvent::IceCandidateDiscovered`] event into this peers
    /// `peer_events_sender`.
    fn on_ice_candidate(
        id: Id,
        sender: &mpsc::UnboundedSender<PeerEvent>,
        candidate: IceCandidate,
    ) {
        let _ = sender.unbounded_send(PeerEvent::IceCandidateDiscovered {
            peer_id: id,
            candidate: candidate.candidate,
            sdp_m_line_index: candidate.sdp_m_line_index,
            sdp_mid: candidate.sdp_mid,
        });
    }

    /// Handle `iceconnectionstatechange` event from underlying peer emitting
    /// [`PeerEvent::IceConnectionStateChanged`] event into this peers
    /// `peer_events_sender`.
    fn on_ice_connection_state_changed(
        peer_id: Id,
        sender: &mpsc::UnboundedSender<PeerEvent>,
        ice_connection_state: RtcIceConnectionState,
    ) {
        use RtcIceConnectionState::*;

        let ice_connection_state = match ice_connection_state {
            New => IceConnectionState::New,
            Checking => IceConnectionState::Checking,
            Connected => IceConnectionState::Connected,
            Completed => IceConnectionState::Completed,
            Failed => IceConnectionState::Failed,
            Disconnected => IceConnectionState::Disconnected,
            Closed => IceConnectionState::Closed,
            _ => {
                console_error("Unknown ICE connection state");
                return;
            }
        };

        let _ = sender.unbounded_send(PeerEvent::IceConnectionStateChanged {
            peer_id,
            ice_connection_state,
        });
    }

    /// Handle `track` event from underlying peer adding new track to
    /// `media_connections` and emitting [`PeerEvent::NewRemoteStream`]
    /// event into this peers `peer_events_sender` if all tracks from this
    /// sender has arrived.
    fn on_track(
        id: Id,
        media_connections: &MediaConnections,
        sender: &mpsc::UnboundedSender<PeerEvent>,
        track_event: &RtcTrackEvent,
    ) {
        let transceiver = track_event.transceiver();
        let track = track_event.track();

        if let Some(sender_id) =
            media_connections.add_remote_track(transceiver, track)
        {
            if let Some(tracks) =
                media_connections.get_tracks_by_sender(sender_id)
            {
                // got all tracks from this sender, so emit
                // PeerEvent::NewRemoteStream
                let _ = sender.unbounded_send(PeerEvent::NewRemoteStream {
                    peer_id: id,
                    sender_id,
                    remote_stream: MediaStream::from_tracks(tracks),
                });
            };
        } else {
            // TODO: means that this peer is out of sync, should be
            //       handled somehow (propagated to medea to init peer
            //       recreation?)
        }
    }

    /// Returns `true` if all [`Sender`]s audio tracks are enabled.
    pub fn is_send_audio_enabled(&self) -> bool {
        self.media_connections.is_send_audio_enabled()
    }

    /// Returns `true` if all [`Sender`]s video tracks are enabled.
    pub fn is_send_video_enabled(&self) -> bool {
        self.media_connections.is_send_video_enabled()
    }

    /// Returns all [`Sender`]s from this [`PeerConnection`] with provided
    /// [`TransceiverKind`].
    #[inline]
    pub fn get_senders(&self, kind: TransceiverKind) -> Vec<Rc<Sender>> {
        self.media_connections.get_senders(kind)
    }

    /// Track id to mid relations of all send tracks of this
    /// [`RtcPeerConnection`]. mid is id of [`m= section`][1]. mids are received
    /// directly from registered [`RTCRtpTransceiver`][2]s, and are being
    /// allocated on sdp update.
    ///
    /// # Errors
    ///
    /// Errors if finds transceiver without mid, so must be called after setting
    /// local description if offerer, and remote if answerer.
    ///
    /// [1]: https://tools.ietf.org/html/rfc4566#section-5.14
    /// [2]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    #[inline]
    pub fn get_mids(&self) -> Result<HashMap<TrackId, String>> {
        let mids = self
            .media_connections
            .get_mids()
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(mids)
    }

    /// Sync provided tracks creating all required `Sender`s and
    /// `Receiver`s, request local stream if required, get, set and return
    /// sdp offer.
    pub async fn get_offer(
        &self,
        tracks: Vec<Track>,
        local_stream: Option<&SysMediaStream>,
    ) -> Result<String> {
        self.media_connections
            .update_tracks(tracks)
            .map_err(tracerr::map_from_and_wrap!())?;

        self.insert_local_stream(local_stream)
            .await
            .map_err(tracerr::wrap!())?;

        let offer = self
            .peer
            .create_and_set_offer()
            .await
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(offer)
    }

    /// Replaces local stream in the underlying [RTCPeerConnection][1]
    /// with a provided [MediaStream][2] if its have all required tracks.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    #[inline]
    pub async fn inject_local_stream(
        &self,
        local_stream: &SysMediaStream,
    ) -> Result<()> {
        self.insert_local_stream(Some(local_stream))
            .await
            .map_err(tracerr::wrap!())
    }

    /// Inserts provided [MediaStream][1] into underlying [RTCPeerConnection][2]
    /// if it has all required tracks.
    /// Requests local stream from [`MediaManager`] if no stream was provided.
    /// Will produce [`PeerEvent::NewLocalStream`] if new stream was received
    /// from [`MediaManager`].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    async fn insert_local_stream(
        &self,
        local_stream: Option<&SysMediaStream>,
    ) -> Result<()> {
        if let Some(request) = self.media_connections.get_stream_request() {
            let caps = SimpleStreamRequest::try_from(request)
                .map_err(tracerr::from_and_wrap!())?;
            let (stream, is_new_stream) = if let Some(stream) = local_stream {
                (
                    caps.parse_stream(stream)
                        .map_err(tracerr::map_from_and_wrap!())?,
                    false,
                )
            } else {
                let (stream, is_new) = self
                    .media_manager
                    .get_stream(&caps)
                    .await
                    .map_err(tracerr::map_from_and_wrap!())?;
                (
                    caps.parse_stream(&stream)
                        .map_err(tracerr::map_from_and_wrap!())?,
                    is_new,
                )
            };
            self.media_connections
                .insert_local_stream(&stream)
                .await
                .map_err(tracerr::map_from_and_wrap!())?;
            if is_new_stream {
                let _ = self.peer_events_sender.unbounded_send(
                    PeerEvent::NewLocalStream {
                        peer_id: self.id,
                        local_stream: stream,
                    },
                );
            }
        }
        Ok(())
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP from answer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub async fn set_remote_answer(&self, answer: String) -> Result<()> {
        self.set_remote_description(SdpType::Answer(answer))
            .await
            .map_err(tracerr::wrap!())
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP from offer.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    async fn set_remote_offer(&self, offer: String) -> Result<()> {
        self.set_remote_description(SdpType::Offer(offer))
            .await
            .map_err(tracerr::wrap!())
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP with given
    /// description.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    async fn set_remote_description(&self, desc: SdpType) -> Result<()> {
        self.peer
            .set_remote_description(desc)
            .await
            .map_err(tracerr::map_from_and_wrap!())?;
        *self.has_remote_description.borrow_mut() = true;

        let mut candidates = self.ice_candidates_buffer.borrow_mut();
        let mut futures = Vec::with_capacity(candidates.len());
        while let Some(candidate) = candidates.pop() {
            let peer = Rc::clone(&self.peer);
            futures.push(async move {
                peer.add_ice_candidate(
                    &candidate.candidate,
                    candidate.sdp_m_line_index,
                    &candidate.sdp_mid,
                )
                .await
            });
        }
        future::try_join_all(futures)
            .await
            .map_err(tracerr::map_from_and_wrap!())?;
        Ok(())
    }

    /// Sync provided tracks creating all required `Sender`s and
    /// `Receiver`s, request local stream if required, get, set and return
    /// SDP answer.
    /// `set_remote_description` will create all transceivers and fire all
    /// `on_track` events, so it updates `Receiver`s before
    /// `set_remote_description` and update `Sender`s after.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub async fn process_offer(
        &self,
        offer: String,
        tracks: Vec<Track>,
        local_stream: Option<&SysMediaStream>,
    ) -> Result<String> {
        // TODO: use drain_filter when its stable
        let (recv, send): (Vec<_>, Vec<_>) =
            tracks.into_iter().partition(|track| match track.direction {
                Direction::Send { .. } => false,
                Direction::Recv { .. } => true,
            });

        // update receivers
        self.media_connections
            .update_tracks(recv)
            .map_err(tracerr::map_from_and_wrap!())?;

        self.set_remote_offer(offer)
            .await
            .map_err(tracerr::wrap!())?;

        self.media_connections
            .update_tracks(send)
            .map_err(tracerr::map_from_and_wrap!())?;

        self.insert_local_stream(local_stream)
            .await
            .map_err(tracerr::wrap!())?;

        let answer = self
            .peer
            .create_and_set_answer()
            .await
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(answer)
    }

    /// Adds remote peers [ICE Candidate][1] to this peer.
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-2
    pub async fn add_ice_candidate(
        &self,
        candidate: String,
        sdp_m_line_index: Option<u16>,
        sdp_mid: Option<String>,
    ) -> Result<()> {
        if *self.has_remote_description.borrow() {
            self.peer
                .add_ice_candidate(&candidate, sdp_m_line_index, &sdp_mid)
                .await
                .map_err(tracerr::map_from_and_wrap!())?;
        } else {
            self.ice_candidates_buffer.borrow_mut().push(IceCandidate {
                candidate,
                sdp_m_line_index,
                sdp_mid,
            });
        }
        Ok(())
    }
}

impl Drop for PeerConnection {
    /// Drops `on_track` and `on_ice_candidate` callbacks to prevent leak.
    fn drop(&mut self) {
        let _ = self.peer.on_track::<Box<dyn FnMut(RtcTrackEvent)>>(None);
        let _ = self
            .peer
            .on_ice_candidate::<Box<dyn FnMut(IceCandidate)>>(None);
    }
}
