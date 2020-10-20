//! Adapters to [RTCPeerConnection][1] and related objects.
//!
//! [1]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface

mod conn;
mod ice_server;
mod media;
mod repo;
mod stats;
mod tracks_request;
mod transceiver;

use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashMap},
    convert::TryFrom as _,
    hash::{Hash, Hasher},
    rc::Rc,
};

use derive_more::{Display, From};
use futures::{channel::mpsc, future};
use medea_client_api_proto::{
    self as proto, stats::StatId, Direction, IceConnectionState, IceServer,
    MediaSourceKind, MemberId, PeerConnectionState, PeerId as Id, PeerId,
    TrackId,
};
use medea_macro::dispatchable;
use tracerr::Traced;
use web_sys::{RtcIceConnectionState, RtcTrackEvent};

use crate::{
    media::{
        LocalTracksConstraints, MediaKind, MediaManager, MediaManagerError,
        MediaStreamTrack, RecvConstraints,
    },
    utils::{JasonError, JsCaused, JsError},
    MediaStreamSettings,
};

#[cfg(feature = "mockable")]
#[doc(inline)]
pub use self::repo::MockPeerRepository;
#[doc(inline)]
pub use self::{
    conn::{IceCandidate, RTCPeerConnectionError, RtcPeerConnection, SdpType},
    media::{
        MediaConnections, MediaConnectionsError, MuteState,
        MuteStateTransition, Muteable, Receiver, Sender, StableMuteState,
        TrackDirection, TransceiverSide,
    },
    repo::{PeerRepository, Repository},
    stats::RtcStats,
    tracks_request::{SimpleTracksRequest, TracksRequest, TracksRequestError},
    transceiver::TransceiverDirection,
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
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    #[display(fmt = "{}", _0)]
    RtcPeerConnection(#[js(cause)] RTCPeerConnectionError),

    /// Errors that may occur when validating [`TracksRequest`] or
    /// parsing [`MediaStreamTrack`]s.
    #[display(fmt = "{}", _0)]
    TracksRequest(#[js(cause)] TracksRequestError),
}

type Result<T> = std::result::Result<T, Traced<PeerError>>;

#[dispatchable(self: &Self, async_trait(?Send))]
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

    /// [`RtcPeerConnection`] received new [`MediaStreamTrack`] from remote
    /// sender.
    NewRemoteTrack {
        /// Remote `Member` ID.
        sender_id: MemberId,

        /// Received [`MediaStreamTrack`].
        track: MediaStreamTrack,
    },

    /// [`RtcPeerConnection`] sent new local track to remote members.
    NewLocalTrack {
        /// Local [`MediaStreamTrack`] that is sent to remote members.
        local_track: MediaStreamTrack,
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

    /// [`RtcPeerConnection`]'s [connection][1] state changed.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dfn-ice-connection-state
    ConnectionStateChanged {
        /// ID of the [`PeerConnection`] that sends
        /// [`connectionstatechange`][1] event.
        ///
        /// [1]: https://w3.org/TR/webrtc/#event-connectionstatechange
        peer_id: Id,

        /// New [`PeerConnectionState`].
        peer_connection_state: PeerConnectionState,
    },

    /// [`RtcPeerConnection`]'s [`RtcStats`] update.
    StatsUpdate {
        /// ID of the [`PeerConnection`] for which [`RtcStats`] was sent.
        peer_id: Id,

        /// [`RtcStats`] of this [`PeerConnection`].
        stats: RtcStats,
    },

    /// [`RtcPeerConnection`] is signalling that it [`MediaStream`]
    NewLocalStreamRequired {
        /// ID of the [`PeerConnection`] that requested new media stream.
        peer_id: Id,
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

    /// Last hashes of the all [`RtcStat`]s which was already sent to the
    /// server, so we won't duplicate stats that were already sent.
    ///
    /// Stores precomputed hashes, since we don't need access to actual stats
    /// values.
    sent_stats_cache: RefCell<HashMap<StatId, u64>>,

    /// Local media stream constraints used in this [`PeerConnection`].
    send_constraints: LocalTracksConstraints,

    /// Constraints to the [`MediaStream`]s received by this
    /// [`PeerConnection`]. Used to disable or enable media receiving.
    recv_constraints: Rc<RecvConstraints>,
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
        send_constraints: LocalTracksConstraints,
        recv_constraints: Rc<RecvConstraints>,
    ) -> Result<Rc<Self>> {
        let peer = Rc::new(
            RtcPeerConnection::new(ice_servers, is_force_relayed)
                .map_err(tracerr::map_from_and_wrap!())?,
        );
        let media_connections = Rc::new(MediaConnections::new(
            id,
            Rc::clone(&peer),
            peer_events_sender.clone(),
        ));

        let peer = Self {
            id,
            peer,
            media_connections,
            media_manager,
            peer_events_sender,
            sent_stats_cache: RefCell::new(HashMap::new()),
            has_remote_description: RefCell::new(false),
            ice_candidates_buffer: RefCell::new(Vec::new()),
            send_constraints,
            recv_constraints,
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

        // Bind to `connectionstatechange` event.
        let id = peer.id;
        let sender = peer.peer_events_sender.clone();
        peer.peer
            .on_connection_state_change(Some(move |peer_connection_state| {
                let _ =
                    sender.unbounded_send(PeerEvent::ConnectionStateChanged {
                        peer_id: id,
                        peer_connection_state,
                    });
            }))
            .map_err(tracerr::map_from_and_wrap!())?;

        // Bind to `track` event.
        let media_connections = Rc::clone(&peer.media_connections);
        peer.peer
            .on_track(Some(move |track_event| {
                if let Err(err) =
                    media_connections.add_remote_track(&track_event)
                {
                    JasonError::from(err).print();
                };
            }))
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(Rc::new(peer))
    }

    /// Stops inner state transitions expiry timers.
    ///
    /// Inner state transitions initiated via external APIs that can not be
    /// performed immediately (must wait for Medea notification/approval) have
    /// TTL, after which they are cancelled.
    ///
    /// In some cases you might want to stop expiry timers, e.g. when
    /// connection to Medea is temporary lost.
    ///
    /// This currently affects only [`Senders`] mute/unmute transitions.
    pub fn stop_state_transitions_timers(&self) {
        self.media_connections.stop_state_transitions_timers()
    }

    /// Resets inner state transitions expiry timers.
    ///
    /// Inner state transitions initiated via external APIs that can not be
    /// performed immediately (must wait for Medea notification/approval) have
    /// TTL, after which they are cancelled.
    ///
    /// In some cases you might want to stop expiry timers, e.g. when
    /// connection to Medea is temporary lost.
    ///
    /// This currently affects only [`Senders`] mute/unmute transitions.
    pub fn reset_state_transitions_timers(&self) {
        self.media_connections.reset_state_transitions_timers();
    }

    /// Filters out already sent stats, and send new statss from
    /// provided [`RtcStats`].
    #[allow(clippy::option_if_let_else)]
    pub fn send_peer_stats(&self, stats: RtcStats) {
        let mut stats_cache = self.sent_stats_cache.borrow_mut();
        let stats = RtcStats(
            stats
                .0
                .into_iter()
                .filter(|stat| {
                    let mut hasher = DefaultHasher::new();
                    stat.stats.hash(&mut hasher);
                    let stat_hash = hasher.finish();

                    if let Some(last_hash) = stats_cache.get_mut(&stat.id) {
                        if *last_hash == stat_hash {
                            false
                        } else {
                            *last_hash = stat_hash;
                            true
                        }
                    } else {
                        stats_cache.insert(stat.id.clone(), stat_hash);
                        true
                    }
                })
                .collect(),
        );

        if !stats.0.is_empty() {
            let _ = self.peer_events_sender.unbounded_send(
                PeerEvent::StatsUpdate {
                    peer_id: self.id,
                    stats,
                },
            );
        }
    }

    /// Sends [`RtcStats`] update of this [`PeerConnection`] to the server.
    pub async fn scrape_and_send_peer_stats(&self) {
        match self.peer.get_stats().await {
            Ok(stats) => self.send_peer_stats(stats),
            Err(e) => {
                JasonError::from(e).print();
            }
        };
    }

    /// Returns [`RtcStats`] of this [`PeerConnection`].
    ///
    /// # Errors
    ///
    /// Errors with [`PeerError::RtcPeerConnection`] if failed to get
    /// [`RtcStats`].
    pub async fn get_stats(&self) -> Result<RtcStats> {
        self.peer
            .get_stats()
            .await
            .map_err(tracerr::map_from_and_wrap!())
    }

    /// Returns `true` if all [`TransceiverSide`]s with a provided
    /// [`MediaKind`], [`TrackDirection`] and [`MediaSourceKind`] is in the
    /// provided [`StableMuteState`].
    #[inline]
    pub fn is_all_transceiver_sides_in_mute_state(
        &self,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
        mute_state: StableMuteState,
    ) -> bool {
        self.media_connections.is_all_tracks_in_mute_state(
            kind,
            direction,
            source_kind,
            mute_state,
        )
    }

    /// Returns [`PeerId`] of this [`PeerConnection`].
    #[inline]
    pub fn id(&self) -> PeerId {
        self.id
    }

    /// Updates [`Sender`]s and [`Receiver`]s of this [`PeerConnection`] with
    /// [`proto::TrackPatch`].
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::InvalidTrackPatch`] if
    /// provided [`proto::TrackPatch`] contains unknown ID.
    pub fn patch_tracks(
        &self,
        tracks: Vec<proto::TrackPatchEvent>,
    ) -> Result<()> {
        Ok(self
            .media_connections
            .patch_tracks(tracks)
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
        use RtcIceConnectionState as S;

        let ice_connection_state = match ice_connection_state {
            S::New => IceConnectionState::New,
            S::Checking => IceConnectionState::Checking,
            S::Connected => IceConnectionState::Connected,
            S::Completed => IceConnectionState::Completed,
            S::Failed => IceConnectionState::Failed,
            S::Disconnected => IceConnectionState::Disconnected,
            S::Closed => IceConnectionState::Closed,
            S::__Nonexhaustive => {
                log::error!("Unknown ICE connection state");
                return;
            }
        };

        let _ = sender.unbounded_send(PeerEvent::IceConnectionStateChanged {
            peer_id,
            ice_connection_state,
        });
    }

    /// Marks [`PeerConnection`] to trigger ICE restart.
    ///
    /// After this function returns, the offer returned by the next call to
    /// [`PeerConnection::get_offer`] is automatically configured to trigger ICE
    /// restart.
    pub fn restart_ice(&self) {
        self.peer.restart_ice();
    }

    /// Returns `true` if all [`Sender`]s audio tracks are enabled.
    #[cfg(feature = "mockable")]
    pub fn is_send_audio_enabled(&self) -> bool {
        self.media_connections.is_send_audio_enabled()
    }

    /// Returns `true` if all [`Sender`]s video tracks are enabled.
    #[cfg(feature = "mockable")]
    pub fn is_send_video_enabled(
        &self,
        source_kind: Option<MediaSourceKind>,
    ) -> bool {
        self.media_connections.is_send_video_enabled(source_kind)
    }

    /// Returns `true` if all [`Receiver`]s audio tracks are enabled.
    pub fn is_recv_audio_enabled(&self) -> bool {
        self.media_connections.is_recv_audio_enabled()
    }

    /// Returns `true` if all [`Receiver`]s video tracks are enabled.
    pub fn is_recv_video_enabled(&self) -> bool {
        self.media_connections.is_recv_video_enabled()
    }

    /// Returns all [`TransceiverSide`]s from this [`PeerConnection`] with
    /// provided [`MediaKind`], [`TrackDirection`] and [`MediaSourceKind`].
    #[inline]
    pub fn get_transceivers_sides(
        &self,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
    ) -> Vec<Rc<dyn TransceiverSide>> {
        self.media_connections.get_transceivers_sides(
            kind,
            direction,
            source_kind,
        )
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
    /// [2]: https://w3.org/TR/webrtc/#rtcrtptransceiver-interface
    #[inline]
    pub fn get_mids(&self) -> Result<HashMap<TrackId, String>> {
        let mids = self
            .media_connections
            .get_mids()
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(mids)
    }

    /// Returns publishing statuses of the all [`Sender`]s from this
    /// [`MediaConnections`].
    pub fn get_transceivers_statuses(&self) -> HashMap<TrackId, bool> {
        self.media_connections.get_transceivers_statuses()
    }

    /// Syncs provided tracks creating all required `Sender`s and `Receiver`s,
    /// request local stream if required, get, set and return SDP offer.
    ///
    /// # Errors
    ///
    /// With [`MediaConnectionsError::TransceiverNotFound`] if cannot find
    /// transceiver by `mid`.
    ///
    /// With [`RTCPeerConnectionError::CreateOfferFailed`] if
    /// [RtcPeerConnection.createOffer()][1] fails.
    ///
    /// With [`RTCPeerConnectionError::SetLocalDescriptionFailed`] if
    /// [RtcPeerConnection.setLocalDescription()][2] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection-createoffer
    /// [2]: https://w3.org/TR/webrtc/#dom-peerconnection-setlocaldescription
    pub async fn get_offer(&self, tracks: Vec<proto::Track>) -> Result<String> {
        self.media_connections
            .create_tracks(
                tracks,
                &self.send_constraints,
                &self.recv_constraints,
            )
            .map_err(tracerr::map_from_and_wrap!())?;

        self.update_local_stream().await.map_err(tracerr::wrap!())?;

        let offer = self
            .peer
            .create_and_set_offer()
            .await
            .map_err(tracerr::map_from_and_wrap!())?;
        Ok(offer)
    }

    /// Creates new [`Sender`]s and [`Receiver`]s for each new [`Track`].
    ///
    /// # Errors
    ///
    /// With [`MediaConnectionsError::TransceiverNotFound`] if could not create
    /// new [`Sender`] because transceiver with specified `mid` doesn't exist.
    pub fn create_tracks(&self, tracks: Vec<proto::Track>) -> Result<()> {
        self.media_connections
            .create_tracks(
                tracks,
                &self.send_constraints,
                &self.recv_constraints,
            )
            .map_err(tracerr::map_from_and_wrap!())?;
        Ok(())
    }

    /// Updates local [MediaStream][1] being used in [`PeerConnection`]
    /// [`Sender`]s.
    ///
    /// First of all make sure that [`PeerConnection`] [`Sender`]s are up to
    /// date (you set those with [`PeerConnection::create_tracks`]). If
    /// there are no senders configured in this [`PeerConnection`], then
    /// this method is no-op.
    ///
    /// Secondly, make sure that configured [`LocalStreamConstraints`] are up to
    /// date.
    ///
    /// This function requests local stream from [`MediaManager`]. If stream
    /// returned from [`MediaManager`] is considered new, then this function
    /// will emit [`PeerEvent::NewLocalStream`] event.
    ///
    /// Constraints being used when requesting stream from [`MediaManager`] are
    /// a result of merging constraints received from this [`PeerConnection`]
    /// [`Sender`]s, which are configured by server during signalling, and
    /// [`LocalStreamConstraints`], that are optionally configured by JS-side.
    ///
    /// Returns [`HashMap`] with [`MuteState`]s updates for the [`Sender`]s.
    ///
    /// # Errors
    ///
    /// With [`TracksRequestError`] if current state of peer's [`Sender`]s
    /// cannot be represented as [`SimpleTracksRequest`] (max 1 audio [`Sender`]
    /// and max 1 video [`Sender`]), or [`MediaStream`] requested from
    /// [`MediaManager`] does not satisfy [`Sender`]s constraints.
    ///
    /// With [`TracksRequestError::ExpectedAudioTracks`] or
    /// [`TracksRequestError::ExpectedVideoTracks`] if provided
    /// [`MediaStreamSettings`] are incompatible with this peer [`Sender`]s
    /// constraints.
    ///
    /// With [`MediaManagerError::GetUserMediaFailed`] or
    /// [`MediaManagerError::GetDisplayMediaFailed`] if corresponding request to
    /// UA failed.
    ///
    /// With [`MediaConnectionsError::InvalidMediaTracks`],
    /// [`MediaConnectionsError::InvalidMediaTrack`] or
    /// [`MediaConnectionsError::CouldNotInsertLocalTrack`] if [`MediaStream`]
    /// cannot be inserted into peer's [`Sender`]s.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface
    pub async fn update_local_stream(
        &self,
    ) -> Result<HashMap<TrackId, StableMuteState>> {
        if let Some(request) = self.media_connections.get_tracks_request() {
            let mut required_caps = SimpleTracksRequest::try_from(request)
                .map_err(tracerr::from_and_wrap!())?;

            required_caps
                .merge(self.send_constraints.inner())
                .map_err(tracerr::map_from_and_wrap!())?;

            let used_caps = MediaStreamSettings::from(&required_caps);

            let media_tracks = self
                .media_manager
                .get_tracks(used_caps)
                .await
                .map_err(tracerr::map_from_and_wrap!())?;
            let peer_tracks = required_caps
                .parse_tracks(
                    media_tracks.iter().map(|(t, _)| t).cloned().collect(),
                )
                .map_err(tracerr::map_from_and_wrap!())?;

            let mute_states_updates = self
                .media_connections
                .insert_local_tracks(&peer_tracks)
                .await
                .map_err(tracerr::map_from_and_wrap!())?;

            for (track, is_new) in media_tracks {
                if is_new {
                    let _ = self.peer_events_sender.unbounded_send(
                        PeerEvent::NewLocalTrack { local_track: track },
                    );
                }
            }

            Ok(mute_states_updates)
        } else {
            Ok(HashMap::new())
        }
    }

    /// Returns [`Rc`] to [`TransceiverSide`] with a provided [`TrackId`].
    ///
    /// Returns [`None`] if [`TransceiverSide`] with a provided [`TrackId`]
    /// doesn't exist in this [`PeerConnection`].
    pub fn get_transceiver_side_by_id(
        &self,
        track_id: TrackId,
    ) -> Option<Rc<dyn TransceiverSide>> {
        self.media_connections.get_transceiver_side_by_id(track_id)
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP from answer.
    ///
    /// # Errors
    ///
    /// With [`RTCPeerConnectionError::SetRemoteDescriptionFailed`] if
    /// [RTCPeerConnection.setRemoteDescription()][2] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://w3.org/TR/webrtc/#dom-peerconnection-setremotedescription
    pub async fn set_remote_answer(&self, answer: String) -> Result<()> {
        self.set_remote_description(SdpType::Answer(answer))
            .await
            .map_err(tracerr::wrap!())
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP from offer.
    ///
    /// # Errors
    ///
    /// With [`RTCPeerConnectionError::SetRemoteDescriptionFailed`] if
    /// [RTCPeerConnection.setRemoteDescription()][2] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://w3.org/TR/webrtc/#dom-peerconnection-setremotedescription
    async fn set_remote_offer(&self, offer: String) -> Result<()> {
        self.set_remote_description(SdpType::Offer(offer))
            .await
            .map_err(tracerr::wrap!())
    }

    /// Updates underlying [RTCPeerConnection][1]'s remote SDP with given
    /// description.
    ///
    /// # Errors
    ///
    /// With [`RTCPeerConnectionError::SetRemoteDescriptionFailed`] if
    /// [RTCPeerConnection.setRemoteDescription()][2] fails.
    ///
    /// With [`RTCPeerConnectionError::AddIceCandidateFailed`] if
    /// [RtcPeerConnection.addIceCandidate()][3] fails when adding buffered ICE
    /// candidates.
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://w3.org/TR/webrtc/#dom-peerconnection-setremotedescription
    /// [3]: https://w3.org/TR/webrtc/#dom-peerconnection-addicecandidate
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
    /// # Errors
    ///
    /// With [`MediaConnectionsError::TransceiverNotFound`] if cannot create
    /// new [`Sender`] because of transceiver with specified `mid` being absent.
    ///
    /// With [`RTCPeerConnectionError::SetRemoteDescriptionFailed`] if
    /// [RTCPeerConnection.setRemoteDescription()][2] fails.
    ///
    /// With [`TracksRequestError`], [`MediaManagerError`] or
    /// [`MediaConnectionsError`] if cannot get or insert [`MediaStream`].
    ///
    /// With [`RTCPeerConnectionError::CreateAnswerFailed`] if
    /// [RtcPeerConnection.createAnswer()][3] fails.
    ///
    /// With [`RTCPeerConnectionError::SetLocalDescriptionFailed`] if
    /// [RtcPeerConnection.setLocalDescription()][4] fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtcpeerconnection-interface
    /// [2]: https://w3.org/TR/webrtc/#dom-peerconnection-setremotedescription
    /// [3]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection-createanswer
    /// [4]: https://w3.org/TR/webrtc/#dom-peerconnection-setlocaldescription
    pub async fn process_offer(
        &self,
        offer: String,
        tracks: Vec<proto::Track>,
    ) -> Result<String> {
        // TODO: use drain_filter when its stable
        let (recv, send): (Vec<_>, Vec<_>) =
            tracks.into_iter().partition(|track| match track.direction {
                Direction::Send { .. } => false,
                Direction::Recv { .. } => true,
            });

        // create receivers
        self.media_connections
            .create_tracks(recv, &self.send_constraints, &self.recv_constraints)
            .map_err(tracerr::map_from_and_wrap!())?;

        // set offer, which will create transceivers and discover remote tracks
        // in receivers
        self.set_remote_offer(offer)
            .await
            .map_err(tracerr::wrap!())?;

        // create senders
        self.media_connections
            .create_tracks(send, &self.send_constraints, &self.recv_constraints)
            .map_err(tracerr::map_from_and_wrap!())?;

        self.update_local_stream().await.map_err(tracerr::wrap!())?;

        let answer = self
            .peer
            .create_and_set_answer()
            .await
            .map_err(tracerr::map_from_and_wrap!())?;

        Ok(answer)
    }

    /// Adds remote peers [ICE Candidate][1] to this peer.
    ///
    /// # Errors
    ///
    /// With [`RTCPeerConnectionError::AddIceCandidateFailed`] if
    /// [RtcPeerConnection.addIceCandidate()][2] fails to add buffered
    /// [ICE candidates][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-2
    /// [2]: https://w3.org/TR/webrtc/#dom-peerconnection-addicecandidate
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
