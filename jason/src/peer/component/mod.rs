//! Implementation of the [`PeerComponent`].

mod ice_candidates;
mod local_sdp;
mod tracks_repository;
mod watchers;

use std::{collections::HashSet, rc::Rc};

use medea_client_api_proto::{
    state as proto_state, IceCandidate, IceServer, NegotiationRole, PeerId,
    TrackId,
};
use medea_reactive::{
    collections::ProgressableHashMap, ObservableCell, ProgressableCell,
    RecheckableFutureExt,
};
use tracerr::Traced;

use crate::{
    peer::{
        media::{ReceiverState, SenderState},
        LocalStreamUpdateCriteria, PeerConnection, PeerError,
    },
    utils::{AsProtoState, Component, SynchronizableState, Updatable},
};

use self::{
    ice_candidates::IceCandidates, local_sdp::LocalSdp,
    tracks_repository::TracksRepository,
};

/// Component responsible for the [`PeerConnection`] updating.
pub type PeerComponent = Component<PeerState, PeerConnection>;

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
enum SyncState {
    /// State desynced, and should be synced on RPC reconnect.
    Desynced,

    /// State syncs with a Media Server state.
    Syncing,

    /// State is synced.
    Synced,
}

/// State of the [`PeerComponent`].
#[derive(Debug)]
pub struct PeerState {
    /// ID of the [`PeerComponent`].
    id: PeerId,

    /// All [`SenderState`]s of this [`PeerComponent`].
    senders: TracksRepository<SenderState>,

    /// All [`ReceiverState`]s of this [`PeerComponent`].
    receivers: TracksRepository<ReceiverState>,

    /// Flag which indicates that this [`PeerComponent`] should relay all media
    /// through a TURN server forcibly.
    force_relay: bool,

    /// List of [`IceServer`]s which this [`PeerComponent`] should use.
    ice_servers: Vec<IceServer>,

    /// Current [`NegotiationRole`] of this [`PeerComponent`].
    negotiation_role: ObservableCell<Option<NegotiationRole>>,

    /// Negotiation state of the [`PeerComponent`].
    negotiation_state: ObservableCell<NegotiationState>,

    /// Current SDP offer of this [`PeerComponent`].
    sdp_offer: LocalSdp,

    /// Current SDP offer of the partner [`PeerComponent`].
    remote_sdp_offer: ProgressableCell<Option<String>>,

    /// Flag which indicates that ICE restart should be performed.
    restart_ice: ObservableCell<bool>,

    /// All [`IceCandidate`]s of this [`PeerComponent`].
    ice_candidates: IceCandidates,

    /// Synchronization state of the [`PeerComponent`].
    sync_state: ObservableCell<SyncState>,
}

impl PeerState {
    /// Returns [`PeerState`] with a provided data.
    #[inline]
    pub fn new(
        id: PeerId,
        senders: ProgressableHashMap<TrackId, Rc<SenderState>>,
        receivers: ProgressableHashMap<TrackId, Rc<ReceiverState>>,
        ice_servers: Vec<IceServer>,
        force_relay: bool,
        negotiation_role: Option<NegotiationRole>,
        ice_candidates: HashSet<IceCandidate>,
    ) -> Self {
        Self {
            id,
            senders: TracksRepository::new(senders),
            receivers: TracksRepository::new(receivers),
            ice_servers,
            force_relay,
            remote_sdp_offer: ProgressableCell::new(None),
            sdp_offer: LocalSdp::new(),
            negotiation_role: ObservableCell::new(negotiation_role),
            negotiation_state: ObservableCell::new(NegotiationState::Stable),
            restart_ice: ObservableCell::new(false),
            ice_candidates: IceCandidates::from_proto(ice_candidates),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
    }

    /// Returns all [`IceServer`]s of this [`PeerState`].
    #[inline]
    pub fn ice_servers(&self) -> &Vec<IceServer> {
        &self.ice_servers
    }

    /// Returns `true` if `PeerConnection` should be relayed forcibly.
    #[inline]
    pub fn force_relay(&self) -> bool {
        self.force_relay
    }

    /// Inserts new [`SenderState`] into this [`PeerState`].
    #[inline]
    pub fn insert_sender(&self, track_id: TrackId, sender: Rc<SenderState>) {
        self.senders.insert(track_id, sender);
    }

    /// Inserts new [`ReceiverState`] into this [`PeerState`].
    #[inline]
    pub fn insert_receiver(
        &self,
        track_id: TrackId,
        receiver: Rc<ReceiverState>,
    ) {
        self.receivers.insert(track_id, receiver);
    }

    /// Returns [`Rc`] to the [`SenderState`] with a provided [`TrackId`].
    #[inline]
    pub fn get_sender(&self, track_id: TrackId) -> Option<Rc<SenderState>> {
        self.senders.get(track_id)
    }

    /// Returns [`Rc`] to the [`ReceiverState`] with a provided [`TrackId`].
    #[inline]
    pub fn get_receiver(&self, track_id: TrackId) -> Option<Rc<ReceiverState>> {
        self.receivers.get(track_id)
    }

    /// Sets [`NegotiationRole`] of this [`PeerState`] to the provided one.
    #[inline]
    pub async fn set_negotiation_role(
        &self,
        negotiation_role: NegotiationRole,
    ) {
        self.negotiation_role.when_eq(None).await.ok();
        self.negotiation_role.set(Some(negotiation_role));
    }

    /// Sets [`PeerState::restart_ice`] to `true`.
    #[inline]
    pub fn restart_ice(&self) {
        self.restart_ice.set(true);
    }

    /// Sets remote SDP offer to the provided value.
    #[inline]
    pub fn set_remote_sdp_offer(&self, new_remote_sdp_offer: String) {
        self.remote_sdp_offer.set(Some(new_remote_sdp_offer));
    }

    /// Adds [`IceCandidate`] for the [`PeerState`].
    #[inline]
    pub fn add_ice_candidate(&self, ice_candidate: IceCandidate) {
        self.ice_candidates.add(ice_candidate);
    }

    /// Returns current SDP offer of this [`PeerState`].
    #[inline]
    pub fn current_sdp_offer(&self) -> Option<String> {
        self.sdp_offer.current()
    }

    /// Marks current [`LocalSdp`] as approved by server.
    #[inline]
    pub fn sdp_offer_applied(&self, sdp_offer: &str) {
        self.sdp_offer.approve(sdp_offer);
    }

    /// Stops all timeouts of the [`PeerState`].
    ///
    /// Stops local SDP rollback timeout.
    #[inline]
    pub fn stop_timeouts(&self) {
        self.sdp_offer.stop_timeout();
    }

    /// Resumes all timeouts of the [`PeerState`].
    ///
    /// Resumes local SDP rollback timeout.
    #[inline]
    pub fn resume_timeouts(&self) {
        self.sdp_offer.resume_timeout();
    }

    /// Notifies [`PeerComponent`] about RPC connection loss.
    #[inline]
    pub fn connection_lost(&self) {
        self.sync_state.set(SyncState::Desynced);
    }

    /// Notifies [`PeerComponent`] about RPC connection restore.
    #[inline]
    pub fn reconnected(&self) {
        self.sync_state.set(SyncState::Syncing);
    }

    /// Updates local `MediaStream` based on
    /// [`SenderState::is_local_stream_update_needed`].
    ///
    /// Resets [`SenderState`] local stream update when it updated.
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
}

#[cfg(feature = "mockable")]
impl PeerState {
    /// Waits for [`PeerState::remote_sdp_offer`] change apply.
    #[inline]
    pub async fn when_remote_sdp_answer_processed(&self) {
        self.remote_sdp_offer.when_all_processed().await;
    }

    /// Resets [`NegotiationRole`] of this [`PeerState`] to [`None`].
    #[inline]
    pub fn reset_negotiation_role(&self) {
        self.negotiation_role.set(None);
    }
}

impl AsProtoState for PeerState {
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
            sdp_offer: self.sdp_offer.current(),
            remote_sdp_offer: self.remote_sdp_offer.get(),
            restart_ice: self.restart_ice.get(),
        }
    }
}

impl SynchronizableState for PeerState {
    type Input = proto_state::Peer;

    fn from_proto(from: Self::Input) -> Self {
        Self::new(
            from.id,
            from.senders
                .into_iter()
                .map(|(id, sender)| (id, Rc::new(SenderState::from(sender))))
                .collect(),
            from.receivers
                .into_iter()
                .map(|(id, receiver)| {
                    (id, Rc::new(ReceiverState::from(receiver)))
                })
                .collect(),
            from.ice_servers,
            from.force_relay,
            from.negotiation_role,
            from.ice_candidates,
        )
    }

    fn apply(&self, state: Self::Input) {
        if state.negotiation_role.is_some() {
            self.negotiation_role.set(state.negotiation_role);
        }
        if state.restart_ice {
            self.restart_ice.set(true);
        }
        self.sdp_offer.update_offer_by_server(&state.sdp_offer);
        self.remote_sdp_offer.set(state.remote_sdp_offer);
        self.ice_candidates.apply(state.ice_candidates);
        self.senders.apply(state.senders);
        self.receivers.apply(state.receivers);

        self.sync_state.set(SyncState::Synced);
    }
}

impl Updatable for PeerState {
    fn when_updated(&self) -> Box<dyn RecheckableFutureExt<Output = ()>> {
        Box::new(medea_reactive::join_all(vec![
            self.receivers.when_updated(),
            self.senders.when_updated(),
        ]))
    }
}
