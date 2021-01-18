//! Implementation of the [`PeerComponent`].

mod ice_candidates;
mod local_sdp;
mod tracks_repository;
mod watchers;

use std::rc::Rc;

use futures::channel::mpsc;
use medea_client_api_proto::{
    self as proto, state as proto_state, IceCandidate, IceServer,
    NegotiationRole, PeerId, TrackId,
};
use medea_reactive::{ObservableCell, ProgressableCell, AllProcessed};
use tracerr::Traced;

use crate::{
    api::Connections,
    media::{LocalTracksConstraints, MediaManager, RecvConstraints},
    peer::{
        media::{receiver, sender},
        LocalStreamUpdateCriteria, PeerConnection, PeerError, PeerEvent,
    },
    utils::{component, AsProtoState, SynchronizableState, Updatable},
};

use self::{
    ice_candidates::IceCandidates, local_sdp::LocalSdp,
    tracks_repository::TracksRepository,
};
use futures::future::LocalBoxFuture;
use wasm_bindgen_futures::spawn_local;
use crate::utils::delay_for;

/// Component responsible for the [`PeerConnection`] updating.
pub type Component = component::Component<State, PeerConnection>;

/// Negotiation state of the [`PeerComponent`].
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
    /// Means that [`PeerComponent`] is new or negotiation completed.
    Stable,

    /// [`PeerComponent`] waits for local SDP offer generating.
    WaitLocalSdp,

    /// [`PeerComponent`] waits for local SDP approve by server.
    WaitLocalSdpApprove,

    /// [`PeerComponent`] waits for remote SDP offer.
    WaitRemoteSdp,
}

/// Synchronization state of the [`PeerComponent`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncState {
    /// State desynced, and should be synced on RPC reconnect.
    Desynced,

    /// State syncs with a Media Server state.
    Syncing,

    /// State is synced.
    Synced,
}

/// State of the [`PeerComponent`].
#[derive(Debug)]
pub struct State {
    /// ID of the [`PeerComponent`].
    id: PeerId,

    /// All [`sender::State`]s of this [`PeerComponent`].
    senders: TracksRepository<sender::State>,

    /// All [`receiver::State`]s of this [`PeerComponent`].
    receivers: TracksRepository<receiver::State>,

    /// Flag which indicates that this [`PeerComponent`] should relay all media
    /// through a TURN server forcibly.
    force_relay: bool,

    /// List of [`IceServer`]s which this [`PeerComponent`] should use.
    ice_servers: Vec<IceServer>,

    /// Current [`NegotiationRole`] of this [`PeerComponent`].
    negotiation_role: ObservableCell<Option<NegotiationRole>>,

    /// Negotiation state of the [`PeerComponent`].
    negotiation_state: ObservableCell<NegotiationState>,

    local_sdp: LocalSdp,

    remote_sdp: ProgressableCell<Option<String>>,

    /// Flag which indicates that ICE restart should be performed.
    restart_ice: ObservableCell<bool>,

    /// All [`IceCandidate`]s of this [`PeerComponent`].
    ice_candidates: IceCandidates,

    /// Synchronization state of the [`PeerComponent`].
    sync_state: ObservableCell<SyncState>,
}

impl State {
    /// Returns [`State`] with a provided data.
    #[inline]
    pub fn new(
        id: PeerId,
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
            restart_ice: ObservableCell::new(false),
            ice_candidates: IceCandidates::new(),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
    }

    /// Returns [`Id`] of this [`State`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> PeerId {
        self.id
    }

    /// Returns all [`IceServer`]s of this [`State`].
    #[inline]
    pub fn ice_servers(&self) -> &Vec<IceServer> {
        &self.ice_servers
    }

    /// Returns `true` if `PeerConnection` should be relayed forcibly.
    #[inline]
    pub fn force_relay(&self) -> bool {
        self.force_relay
    }

    /// Inserts new [`sender::State`] into this [`State`].
    #[inline]
    pub fn insert_sender(&self, track_id: TrackId, sender: Rc<sender::State>) {
        self.senders.insert(track_id, sender);
    }

    /// Inserts new [`receiver::State`] into this [`State`].
    #[inline]
    pub fn insert_receiver(
        &self,
        track_id: TrackId,
        receiver: Rc<receiver::State>,
    ) {
        self.receivers.insert(track_id, receiver);
    }

    /// Returns [`Rc`] to the [`sender::State`] with a provided [`TrackId`].
    #[inline]
    pub fn get_sender(&self, track_id: TrackId) -> Option<Rc<sender::State>> {
        self.senders.get(track_id)
    }

    /// Returns [`Rc`] to the [`receiver::State`] with a provided [`TrackId`].
    #[inline]
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
        self.negotiation_role.when_eq(None).await.ok();
        self.negotiation_role.set(Some(negotiation_role));
    }

    /// Sets [`State::restart_ice`] to `true`.
    #[inline]
    pub fn restart_ice(&self) {
        self.restart_ice.set(true);
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

    /// Marks current [`LocalSdp`] as approved by server.
    #[inline]
    pub fn apply_local_sdp(&self, sdp: String) {
        self.local_sdp.approved_set(sdp);
    }

    /// Returns current SDP offer of this [`State`].
    #[inline]
    pub fn current_sdp_offer(&self) -> Option<String> {
        self.local_sdp.current()
    }

    /// Marks current [`LocalSdp`] as approved by server.
    #[inline]
    pub fn sdp_offer_applied(&self, sdp_offer: &str) {
        // TODO: take String
        self.local_sdp.approved_set(sdp_offer.to_string());
    }

    /// Stops all timeouts of the [`State`].
    ///
    /// Stops [`LocalSdp`] rollback timeout.
    #[inline]
    pub fn stop_timeouts(&self) {
        self.local_sdp.stop_timeout();
    }

    /// Resumes all timeouts of the [`State`].
    ///
    /// Resumes [`LocalSdp`] rollback timeout.
    #[inline]
    pub fn resume_timeouts(&self) {
        self.local_sdp.resume_timeout();
    }

    /// Notifies [`PeerComponent`] about RPC connection loss.
    #[inline]
    pub fn connection_lost(&self) {
        self.sync_state.set(SyncState::Desynced);
        self.senders.connection_lost();
        self.receivers.connection_lost();
    }

    /// Notifies [`PeerComponent`] about RPC connection restore.
    #[inline]
    pub fn reconnected(&self) {
        self.sync_state.set(SyncState::Syncing);
        self.senders.connection_recovered();
        self.receivers.connection_recovered();
    }

    /// Updates local `MediaStream` based on
    /// [`sender::State::is_local_stream_update_needed`].
    ///
    /// Resets [`sender::State`] local stream update when it updated.
    async fn update_local_stream(
        &self,
        ctx: &Rc<PeerConnection>,
    ) -> Result<(), Traced<PeerError>> {
        let mut criteria = LocalStreamUpdateCriteria::empty();
        let senders = self.senders.get_outdated();
        for s in &senders {
            criteria.add(s.media_kind(), s.media_source());
        }
        ctx.update_local_stream(criteria)
            .await
            .map_err(tracerr::map_from_and_wrap!())?;
        for s in senders {
            s.local_stream_updated();
        }

        Ok(())
    }

    /// Inserts provided [`proto::Track`] to this [`State`].
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

    /// Returns [`RecheckableFutureExt`] which will be resolved when all
    /// [`State::senders`]'s inserts/removes will be processed.
    #[inline]
    #[must_use]
    fn when_all_senders_processed(
        &self,
    ) -> AllProcessed<'static> {
        self.senders.when_all_processed()
    }

    /// Returns [`RecheckableFutureExt`] which will be resolved when all
    /// [`State::receivers`]'s inserts/removes will be processed.
    #[inline]
    #[must_use]
    fn when_all_receivers_processed(
        &self,
    ) -> AllProcessed<'static> {
        self.receivers.when_all_processed()
    }

    /// Returns [`Future`] which will be resolved when all [`State::receivers`]
    /// will be stabilized.
    fn when_all_receivers_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        self.senders.when_stabilized()
    }

    /// Returns [`Future`] which will be resolved when all [`State::senders`]
    /// will be stabilized.
    fn when_all_senders_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        self.receivers.when_stabilized()
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

    /// Patches [`sender::State`] or [`receiver::State`] with a provided
    /// [`proto::TrackPatchEvent`].
    pub fn patch_track(&self, track_patch: &proto::TrackPatchEvent) {
        if let Some(sender) = self.get_sender(track_patch.id) {
            sender.update(track_patch);
        } else if let Some(receiver) = self.get_receiver(track_patch.id) {
            receiver.update(track_patch);
        }
    }
}

#[cfg(feature = "mockable")]
impl State {
    /// Waits for [`State::remote_sdp_offer`] change apply.
    #[inline]
    pub async fn when_remote_sdp_answer_processed(&self) {
        self.remote_sdp_offer.when_all_processed().await;
    }

    /// Resets [`NegotiationRole`] of this [`State`] to [`None`].
    #[inline]
    pub fn reset_negotiation_role(&self) {
        self.negotiation_role.set(None);
    }

    /// Waits until [`State::sdp_offer`] will be resolved and returns it's new
    /// value.
    #[inline]
    pub async fn when_local_sdp_offer_updated(&self) -> Option<String> {
        use futures::StreamExt as _;

        self.sdp_offer.subscribe().skip(1).next().await.unwrap()
    }

    /// Waits until all [`State::senders`] and [`State::receivers`] inserts will
    /// be processed.
    #[inline]
    pub async fn when_all_tracks_created(&self) {
        medea_reactive::join_all(vec![
            Box::new(self.senders.when_insert_processed())
                as Box<dyn RecheckableFutureExt<Output = ()>>,
            Box::new(self.receivers.when_insert_processed()),
        ])
        .await;
    }
}

impl AsProtoState for State {
    type Output = proto_state::Peer;

    fn as_proto(&self) -> Self::Output {
        Self::Output {
            id: self.id,
            senders: self.senders.as_proto(),
            receivers: self.receivers.as_proto(),
            ice_candidates: self.ice_candidates.as_proto(),
            force_relay: self.force_relay,
            ice_servers: self.ice_servers.clone(),
            negotiation_role: self.negotiation_role.get(),
            sdp_offer: self.local_sdp.current(),
            remote_sdp_offer: self.remote_sdp.get(),
            restart_ice: self.restart_ice.get(),
        }
    }
}

impl SynchronizableState for State {
    type Input = proto_state::Peer;

    fn from_proto(from: Self::Input) -> Self {
        let state = Self::new(
            from.id,
            from.ice_servers,
            from.force_relay,
            from.negotiation_role,
        );

        for (id, sender) in from.senders {
            state
                .senders
                .insert(id, Rc::new(sender::State::from(sender)));
        }
        for (id, receiver) in from.receivers {
            state
                .receivers
                .insert(id, Rc::new(receiver::State::from(receiver)));
        }
        for ice_candidate in from.ice_candidates {
            state.ice_candidates.add(ice_candidate);
        }

        state
    }

    fn apply(&self, state: Self::Input) {
        if state.negotiation_role.is_some() {
            self.negotiation_role.set(state.negotiation_role);
        }
        if state.restart_ice {
            self.restart_ice.set(true);
        }
        if let Some(sdp_offer) = state.sdp_offer {
            self.local_sdp.approved_set(sdp_offer);
        }
        self.remote_sdp.set(state.remote_sdp_offer);
        self.ice_candidates.apply(state.ice_candidates);
        self.senders.apply(state.senders);
        self.receivers.apply(state.receivers);

        self.sync_state.set(SyncState::Synced);
    }
}

impl Updatable for State {
    fn when_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        use futures::FutureExt as _;
        Box::pin(
            futures::future::join_all(vec![
                self.senders.when_stabilized(),
                self.receivers.when_stabilized(),
            ])
            .map(|_| ()),
        )
    }

    fn when_updated(&self) -> AllProcessed<'static> {
       medea_reactive::when_all_processed(vec![
            self.receivers.when_updated().into(),
            self.senders.when_updated().into(),
        ])
    }
}
