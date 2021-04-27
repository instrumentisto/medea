//! Implementation of a [`Component`].

mod ice_candidates;
mod local_sdp;
mod tracks_repository;
mod watchers;

use std::{cell::Cell, collections::HashSet, rc::Rc};

use futures::{future::LocalBoxFuture, TryFutureExt as _};
use medea_client_api_proto::{
    self as proto, IceCandidate, IceServer, NegotiationRole, PeerId as Id,
    TrackId,
};
use medea_reactive::{AllProcessed, ObservableCell, ProgressableCell};
use tracerr::Traced;

use crate::{
    media::LocalTracksConstraints,
    peer::{
        media::{receiver, sender},
        LocalStreamUpdateCriteria, PeerConnection, PeerError,
    },
    utils::{component, AsProtoState, SynchronizableState, Updatable},
};

use self::{
    ice_candidates::IceCandidates, local_sdp::LocalSdp,
    tracks_repository::TracksRepository,
};

/// Synchronization state of a [`Component`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncState {
    /// State desynced, and should be synced on a RPC reconnection.
    Desynced,

    /// State syncs with a Media Server state.
    Syncing,

    /// State is synced.
    Synced,
}

/// Negotiation state of the [`Component`].
///
/// ```ignore
///           +--------+
///           |        |
/// +-------->+ Stable +<----------+
/// |         |        |           |
/// |         +---+----+           |
/// |             |                |
/// |             v                |
/// |      +------+-------+        |
/// |      |              |        |
/// |      | WaitLocalSdp +<----+  |
/// |      |              |     |  |
/// |      +------+-------+     |  |
/// |             |             |  |
/// |             v             |  |
/// |  +----------+----------+  |  |
/// |  |                     |  |  |
/// +--+ WaitLocalSdpApprove +--+  |
///    |                     |     |
///    +----------+----------+     |
///               |                |
///               v                |
///       +-------+-------+        |
///       |               |        |
///       | WaitRemoteSdp |        |
///       |               |        |
///       +-------+-------+        |
///               |                |
///               |                |
///               +----------------+
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NegotiationState {
    /// [`Component`] is new or negotiation is completed.
    Stable,

    /// [`Component`] waits for a local SDP offer generating.
    WaitLocalSdp,

    /// [`Component`] waits for a local SDP being approved by server.
    WaitLocalSdpApprove,

    /// [`Component`] waits for a remote SDP offer.
    WaitRemoteSdp,
}

/// State of a [`Component`].
#[derive(Debug)]
pub struct State {
    /// ID of this [`Component`].
    id: Id,

    /// All [`sender::State`]s of this [`Component`].
    senders: TracksRepository<sender::State>,

    /// All [`receiver::State`]s of this [`Component`].
    receivers: TracksRepository<receiver::State>,

    /// Indicator whether this [`Component`] should relay all media through a
    /// TURN server forcibly.
    force_relay: bool,

    /// List of [`IceServer`]s which this [`Component`] should use.
    ice_servers: Vec<IceServer>,

    /// Current [`NegotiationRole`] of this [`Component`].
    negotiation_role: ObservableCell<Option<NegotiationRole>>,

    /// Negotiation state of this [`Component`].
    negotiation_state: ObservableCell<NegotiationState>,

    /// Local session description of this [`Component`].
    local_sdp: LocalSdp,

    /// Remote session description of this [`Component`].
    remote_sdp: ProgressableCell<Option<String>>,

    /// Indicates whether ICE restart should be performed.
    restart_ice: Cell<bool>,

    /// All [`IceCandidate`]s of this [`Component`].
    ice_candidates: IceCandidates,

    /// Indicator whether [`State::update_local_stream`] method should be
    /// called if some [`sender`] wants to update a local stream.
    maybe_update_local_stream: ObservableCell<bool>,

    /// Synchronization state of this [`Component`].
    sync_state: ObservableCell<SyncState>,
}

impl State {
    /// Creates a new [`State`] with the provided data.
    #[inline]
    #[must_use]
    pub fn new(
        id: Id,
        ice_servers: Vec<IceServer>,
        force_relay: bool,
        negotiation_role: Option<NegotiationRole>,
    ) -> Self {
        Self {
            id,
            senders: TracksRepository::new(),
            receivers: TracksRepository::new(),
            ice_servers,
            force_relay,
            remote_sdp: ProgressableCell::new(None),
            local_sdp: LocalSdp::new(),
            negotiation_role: ObservableCell::new(negotiation_role),
            negotiation_state: ObservableCell::new(NegotiationState::Stable),
            restart_ice: Cell::new(false),
            ice_candidates: IceCandidates::new(),
            maybe_update_local_stream: ObservableCell::new(false),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
    }

    /// Returns [`Id`] of this [`State`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> Id {
        self.id
    }

    /// Returns all [`IceServer`]s of this [`State`].
    #[inline]
    #[must_use]
    pub fn ice_servers(&self) -> &Vec<IceServer> {
        &self.ice_servers
    }

    /// Indicates whether [`PeerConnection`] should be relayed forcibly.
    #[inline]
    #[must_use]
    pub fn force_relay(&self) -> bool {
        self.force_relay
    }

    /// Inserts a new [`sender::State`] into this [`State`].
    #[inline]
    pub fn insert_sender(&self, track_id: TrackId, sender: Rc<sender::State>) {
        self.senders.insert(track_id, sender);
    }

    /// Inserts a new [`receiver::State`] into this [`State`].
    #[inline]
    pub fn insert_receiver(
        &self,
        track_id: TrackId,
        receiver: Rc<receiver::State>,
    ) {
        self.receivers.insert(track_id, receiver);
    }

    /// Returns [`Rc`] to the [`sender::State`] with the provided [`TrackId`].
    #[inline]
    #[must_use]
    pub fn get_sender(&self, track_id: TrackId) -> Option<Rc<sender::State>> {
        self.senders.get(track_id)
    }

    /// Returns [`Rc`] to the [`receiver::State`] with the provided [`TrackId`].
    #[inline]
    #[must_use]
    pub fn get_receiver(
        &self,
        track_id: TrackId,
    ) -> Option<Rc<receiver::State>> {
        self.receivers.get(track_id)
    }

    /// Sets [`NegotiationRole`] of this [`State`] to the provided one.
    #[inline]
    pub async fn set_negotiation_role(
        &self,
        negotiation_role: NegotiationRole,
    ) {
        let _ = self.negotiation_role.when(Option::is_none).await;
        self.negotiation_role.set(Some(negotiation_role));
    }

    /// Sets [`State::restart_ice`] to `true`.
    #[inline]
    pub fn restart_ice(&self) {
        self.restart_ice.set(true);
    }

    /// Removes [`sender::State`] or [`receiver::State`] with the provided
    /// [`TrackId`].
    #[inline]
    pub fn remove_track(&self, track_id: TrackId) {
        if !self.receivers.remove(track_id) {
            self.senders.remove(track_id);
        }
    }

    /// Sets remote SDP offer to the provided value.
    #[inline]
    pub fn set_remote_sdp(&self, sdp: String) {
        self.remote_sdp.set(Some(sdp));
    }

    /// Adds [`IceCandidate`] for the [`State`].
    #[inline]
    pub fn add_ice_candidate(&self, ice_candidate: IceCandidate) {
        self.ice_candidates.add(ice_candidate);
    }

    /// Marks current local SDP as approved by server.
    #[inline]
    pub fn apply_local_sdp(&self, sdp: String) {
        self.local_sdp.approved_set(sdp);
    }

    /// Stops all timeouts of the [`State`].
    ///
    /// Stops local SDP rollback timeout.
    #[inline]
    pub fn stop_timeouts(&self) {
        self.local_sdp.stop_timeout();
    }

    /// Resumes all timeouts of the [`State`].
    ///
    /// Resumes local SDP rollback timeout.
    #[inline]
    pub fn resume_timeouts(&self) {
        self.local_sdp.resume_timeout();
    }

    /// Returns [`Future`] resolving once
    /// [getUserMedia()][1]/[getDisplayMedia()][2] request for the provided
    /// [`TrackId`]s is resolved.
    ///
    /// [`Result`] returned by this [`Future`] will be the same as the result of
    /// the [getUserMedia()][1]/[getDisplayMedia()][2] request.
    ///
    /// Returns last known [getUserMedia()][1]/[getDisplayMedia()][2] request's
    /// [`Result`], if currently no such requests are running for the provided
    /// [`TrackId`]s.
    ///
    /// [`Future`]: std::future::Future
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub fn local_stream_update_result(
        &self,
        tracks_ids: HashSet<TrackId>,
    ) -> LocalBoxFuture<'static, Result<(), Traced<PeerError>>> {
        Box::pin(
            self.senders
                .local_stream_update_result(tracks_ids)
                .map_err(tracerr::map_from_and_wrap!()),
        )
    }

    /// Returns [`Future`] resolving when all [`sender::State`]'s and
    /// [`receiver::State`]'s updates will be applied.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_all_updated(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![
            self.senders.when_updated().into(),
            self.receivers.when_updated().into(),
        ])
    }

    /// Updates a local `MediaStream` based on a
    /// [`sender::State::is_local_stream_update_needed`].
    ///
    /// Resets a [`sender::State`] local stream update when it's updated.
    async fn update_local_stream(
        &self,
        peer: &Rc<PeerConnection>,
    ) -> Result<(), Traced<PeerError>> {
        let mut criteria = LocalStreamUpdateCriteria::empty();
        let senders = self.senders.get_outdated();
        for s in &senders {
            criteria.add(s.media_kind(), s.media_source());
        }
        let res = peer
            .update_local_stream(criteria)
            .await
            .map_err(tracerr::map_from_and_wrap!())
            .map(drop);
        for s in senders {
            if let Err(err) = res.clone() {
                s.failed_local_stream_update(err);
            } else {
                s.local_stream_updated();
            }
        }
        res
    }

    /// Inserts the provided [`proto::Track`] to this [`State`].
    ///
    /// # Errors
    ///
    /// Errors with [`PeerError::MediaConnections`] if [`sender::State`]
    /// creation fails.
    pub fn insert_track(
        &self,
        track: &proto::Track,
        send_constraints: LocalTracksConstraints,
    ) -> Result<(), Traced<PeerError>> {
        match &track.direction {
            proto::Direction::Send { receivers, mid } => {
                self.senders.insert(
                    track.id,
                    Rc::new(
                        sender::State::new(
                            track.id,
                            mid.clone(),
                            track.media_type.clone(),
                            receivers.clone(),
                            send_constraints,
                        )
                        .map_err(tracerr::map_from_and_wrap!())?,
                    ),
                );
            }
            proto::Direction::Recv { sender, mid } => {
                self.receivers.insert(
                    track.id,
                    Rc::new(receiver::State::new(
                        track.id,
                        mid.clone(),
                        track.media_type.clone(),
                        sender.clone(),
                    )),
                );
            }
        }

        Ok(())
    }

    /// Returns [`Future`] resolving once all [`State::senders`]' inserts and
    /// removes are processed.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    fn when_all_senders_processed(&self) -> AllProcessed<'static> {
        self.senders.when_all_processed()
    }

    /// Returns [`Future`] resolving once all [`State::receivers`]' inserts and
    /// removes are processed.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    fn when_all_receivers_processed(&self) -> AllProcessed<'static> {
        self.receivers.when_all_processed()
    }

    /// Patches [`sender::State`] or [`receiver::State`] with the provided
    /// [`proto::TrackPatchEvent`].
    ///
    /// Schedules a local stream update.
    pub fn patch_track(&self, track_patch: &proto::TrackPatchEvent) {
        if let Some(sender) = self.get_sender(track_patch.id) {
            sender.update(track_patch);
            self.maybe_update_local_stream.set(true);
        } else if let Some(receiver) = self.get_receiver(track_patch.id) {
            receiver.update(track_patch);
        }
    }

    /// Returns the current SDP offer of this [`State`].
    #[inline]
    #[must_use]
    pub fn current_sdp_offer(&self) -> Option<String> {
        self.local_sdp.current()
    }
}

/// Component responsible for a [`PeerConnection`] updating.
pub type Component = component::Component<State, PeerConnection>;

impl AsProtoState for State {
    type Output = proto::state::Peer;

    #[inline]
    fn as_proto(&self) -> Self::Output {
        Self::Output {
            id: self.id,
            senders: self.senders.as_proto(),
            receivers: self.receivers.as_proto(),
            ice_candidates: self.ice_candidates.as_proto(),
            force_relay: self.force_relay,
            ice_servers: self.ice_servers.clone(),
            negotiation_role: self.negotiation_role.get(),
            local_sdp: self.local_sdp.current(),
            remote_sdp: self.remote_sdp.get(),
            restart_ice: self.restart_ice.get(),
        }
    }
}

impl SynchronizableState for State {
    type Input = proto::state::Peer;

    fn from_proto(
        from: Self::Input,
        send_cons: &LocalTracksConstraints,
    ) -> Self {
        let state = Self::new(
            from.id,
            from.ice_servers,
            from.force_relay,
            from.negotiation_role,
        );

        for (id, sender) in from.senders {
            state.senders.insert(
                id,
                Rc::new(sender::State::from_proto(sender, send_cons)),
            );
        }
        for (id, receiver) in from.receivers {
            state.receivers.insert(
                id,
                Rc::new(receiver::State::from_proto(receiver, send_cons)),
            );
        }
        for ice_candidate in from.ice_candidates {
            state.ice_candidates.add(ice_candidate);
        }

        state
    }

    fn apply(&self, state: Self::Input, send_cons: &LocalTracksConstraints) {
        if state.negotiation_role.is_some() {
            self.negotiation_role.set(state.negotiation_role);
        }
        if state.restart_ice {
            self.restart_ice.set(true);
        }
        if let Some(sdp_offer) = state.local_sdp {
            self.local_sdp.approved_set(sdp_offer);
        }
        self.remote_sdp.set(state.remote_sdp);
        self.ice_candidates.apply(state.ice_candidates, send_cons);
        self.senders.apply(state.senders, send_cons);
        self.receivers.apply(state.receivers, send_cons);

        self.sync_state.set(SyncState::Synced);
    }
}

impl Updatable for State {
    #[inline]
    fn when_stabilized(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![
            self.senders.when_stabilized().into(),
            self.receivers.when_stabilized().into(),
        ])
    }

    #[inline]
    fn when_updated(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![
            self.receivers.when_updated().into(),
            self.senders.when_updated().into(),
        ])
    }

    #[inline]
    fn connection_lost(&self) {
        self.sync_state.set(SyncState::Desynced);
        self.senders.connection_lost();
        self.receivers.connection_lost();
    }

    #[inline]
    fn connection_recovered(&self) {
        self.sync_state.set(SyncState::Syncing);
        self.senders.connection_recovered();
        self.receivers.connection_recovered();
    }
}

#[cfg(feature = "mockable")]
impl State {
    /// Waits for a [`State::remote_sdp`] change to be applied.
    #[inline]
    pub async fn when_remote_sdp_processed(&self) {
        self.remote_sdp.when_all_processed().await;
    }

    /// Resets a [`NegotiationRole`] of this [`State`] to [`None`].
    #[inline]
    pub fn reset_negotiation_role(&self) {
        self.negotiation_state.set(NegotiationState::Stable);
        self.negotiation_role.set(None);
    }

    /// Returns the current [`NegotiationRole`] of this [`State`].
    #[inline]
    #[must_use]
    pub fn negotiation_role(&self) -> Option<NegotiationRole> {
        self.negotiation_role.get()
    }

    /// Returns [`Future`] resolving once local SDP approve is needed.
    #[inline]
    pub fn when_local_sdp_approve_needed(
        &self,
    ) -> impl std::future::Future<Output = ()> {
        use futures::FutureExt as _;
        self.negotiation_state
            .when_eq(NegotiationState::WaitLocalSdpApprove)
            .map(drop)
    }

    /// Stabilizes all [`receiver::State`]s of this [`State`].
    #[inline]
    pub fn stabilize_all(&self) {
        self.receivers.stabilize_all();
    }

    /// Waits until a [`State::local_sdp`] is resolved and returns its new
    /// value.
    #[inline]
    pub async fn when_local_sdp_updated(&self) -> Option<String> {
        use futures::StreamExt as _;

        self.local_sdp.subscribe().skip(1).next().await.unwrap()
    }

    /// Waits until all [`State::senders`]' and [`State::receivers`]' inserts
    /// are processed.
    #[inline]
    pub async fn when_all_tracks_created(&self) {
        medea_reactive::when_all_processed(vec![
            self.senders.when_insert_processed().into(),
            self.receivers.when_insert_processed().into(),
        ])
        .await;
    }

    /// Sets [`State::sync_state`] to the [`SyncState::Synced`].
    #[inline]
    pub fn synced(&self) {
        self.senders.synced();
        self.receivers.synced();
        self.sync_state.set(SyncState::Synced);
    }
}
