//! Implementation of the [`PeerComponent`].

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::{future, future::LocalBoxFuture, StreamExt as _};
use medea_client_api_proto::{
    state as proto_state, Command, IceCandidate, IceServer, NegotiationRole,
    PeerId, TrackId,
};
use medea_macro::{watch, watchers};
use medea_reactive::{
    collections::ProgressableHashMap, Guarded, ObservableCell,
    ObservableHashSet, ObservableVec, ProgressableCell,
};
use tracerr::Traced;

use crate::{
    api::GlobalCtx,
    peer::{
        local_sdp::{LocalSdp, Sdp},
        media::{ReceiverState, SenderBuilder, SenderState},
        media_exchange_state, mute_state, LocalStreamUpdateCriteria, PeerError,
        Receiver, ReceiverComponent, SenderComponent,
    },
    utils::Component,
};

use super::PeerConnection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NegotiationState {
    Stable,
    WaitLocalSdp,
    WaitLocalSdpApprove,
    WaitRemoteSdp,
}

use derive_more::From;
use futures::stream::LocalBoxStream;

pub trait AsProtoState {
    type Output;

    fn as_proto(&self) -> Self::Output;
}

pub trait SynchronizableState {
    type Input;

    fn from_proto(input: Self::Input) -> Self;

    fn apply(&self, input: Self::Input);
}

pub trait Updatable {
    fn when_updated(&self) -> LocalBoxFuture<'static, ()>;
}

#[derive(Debug, From)]
struct TracksRepository<S: 'static>(
    RefCell<ProgressableHashMap<TrackId, Rc<S>>>,
);

impl<S> TracksRepository<S> {
    pub fn new(tracks: ProgressableHashMap<TrackId, Rc<S>>) -> Self {
        Self(RefCell::new(tracks))
    }

    pub fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        self.0.borrow().when_all_processed()
    }

    pub fn insert(&self, id: TrackId, track: Rc<S>) {
        self.0.borrow_mut().insert(id, track);
    }

    pub fn get(&self, id: TrackId) -> Option<Rc<S>> {
        self.0.borrow().get(&id).cloned()
    }

    pub fn on_insert(
        &self,
    ) -> LocalBoxStream<'static, Guarded<(TrackId, Rc<S>)>> {
        self.0.borrow().on_insert_with_replay()
    }
}

impl TracksRepository<SenderState> {
    pub fn get_outdated(&self) -> Vec<Rc<SenderState>> {
        self.0
            .borrow()
            .values()
            .filter(|s| s.is_local_stream_update_needed())
            .cloned()
            .collect()
    }
}

impl<S> SynchronizableState for TracksRepository<S>
where
    S: SynchronizableState,
{
    type Input = HashMap<TrackId, S::Input>;

    fn from_proto(input: Self::Input) -> Self {
        Self(RefCell::new(
            input
                .into_iter()
                .map(|(id, t)| (id, Rc::new(S::from_proto(t))))
                .collect(),
        ))
    }

    fn apply(&self, input: Self::Input) {
        for (id, track) in input {
            if let Some(sync_track) = self.0.borrow().get(&id) {
                sync_track.apply(track);
            } else {
                self.0
                    .borrow_mut()
                    .insert(id, Rc::new(S::from_proto(track)));
            }
        }
    }
}

impl<S> Updatable for TracksRepository<S>
where
    S: Updatable,
{
    fn when_updated(&self) -> LocalBoxFuture<'static, ()> {
        let when_futs: Vec<_> =
            self.0.borrow().values().map(|s| s.when_updated()).collect();
        let fut = futures::future::join_all(when_futs);
        Box::pin(async move {
            fut.await;
        })
    }
}

impl<S> AsProtoState for TracksRepository<S>
where
    S: AsProtoState,
{
    type Output = HashMap<TrackId, S::Output>;

    fn as_proto(&self) -> Self::Output {
        self.0
            .borrow()
            .iter()
            .map(|(id, s)| (*id, s.as_proto()))
            .collect()
    }
}

impl Updatable for PeerState {
    fn when_updated(&self) -> LocalBoxFuture<'static, ()> {
        let fut = future::join(
            self.receivers.when_updated(),
            self.senders.when_updated(),
        );
        Box::pin(async move {
            fut.await;
        })
    }
}

/// State of the [`PeerComponent`].
pub struct PeerState {
    id: PeerId,
    senders: TracksRepository<SenderState>,
    receivers: TracksRepository<ReceiverState>,
    ice_servers: Vec<IceServer>,
    force_relay: bool,
    negotiation_role: ObservableCell<Option<NegotiationRole>>,
    negotiation_state: ObservableCell<NegotiationState>,
    sdp_offer: LocalSdp,
    remote_sdp_offer: ProgressableCell<Option<String>>,
    restart_ice: ObservableCell<bool>,
    ice_candidates: RefCell<ObservableHashSet<IceCandidate>>,
}

impl From<&PeerState> for proto_state::PeerState {
    fn from(from: &PeerState) -> Self {
        let ice_candidates =
            from.ice_candidates.borrow().iter().cloned().collect();

        Self {
            id: from.id,
            senders: from.senders.as_proto(),
            receivers: from.receivers.as_proto(),
            ice_candidates,
            force_relay: from.force_relay,
            ice_servers: from.ice_servers.clone(),
            negotiation_role: from.negotiation_role.get(),
            sdp_offer: from.sdp_offer.current(),
            remote_sdp_offer: from.remote_sdp_offer.get(),
            restart_ice: from.restart_ice.get(),
        }
    }
}

impl SynchronizableState for PeerState {
    type Input = proto_state::PeerState;

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
        )
    }

    fn apply(&self, state: Self::Input) {
        if state.negotiation_role.is_some() {
            self.negotiation_role.set(state.negotiation_role);
        }
        if state.restart_ice {
            self.restart_ice.set(true);
        }
        self.sdp_offer.update_offer_by_server(state.sdp_offer);
        self.remote_sdp_offer.set(state.remote_sdp_offer);
        self.ice_candidates
            .borrow_mut()
            .update(state.ice_candidates);

        self.senders.apply(state.senders);
        self.receivers.apply(state.receivers);
    }
}

impl From<proto_state::PeerState> for PeerState {
    fn from(from: proto_state::PeerState) -> Self {
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
        )
    }
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
            ice_candidates: RefCell::new(ObservableHashSet::new()),
        }
    }

    pub fn apply(&self, state: proto_state::PeerState) {
        SynchronizableState::apply(self, state)
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
    pub fn set_negotiation_role(&self, negotiation_role: NegotiationRole) {
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
        self.ice_candidates.borrow_mut().insert(ice_candidate);
    }

    /// Returns current SDP offer of this [`PeerState`].
    #[inline]
    pub fn current_sdp_offer(&self) -> Option<String> {
        self.sdp_offer.current()
    }

    /// Marks current [`LocalSdp`] as approved by server.
    #[inline]
    pub fn sdp_offer_applied(&self) {
        self.sdp_offer.approve();
    }

    /// Stops all timeouts of the [`PeerState`].
    ///
    /// Stops [`LocalSdp`] rollback timeout.
    #[inline]
    pub fn stop_timeouts(&self) {
        self.sdp_offer.stop_timeout();
    }

    /// Resumes all timeouts of the [`PeerState`].
    ///
    /// Resumes [`LocalSdp`] rollback timeout.
    #[inline]
    pub fn resume_timeouts(&self) {
        self.sdp_offer.resume_timeout();
    }

    /// Returns [`Future`] which will be resolved when all
    /// [`SenderState`]s/[`ReceiverState`]s updates will be applied.
    ///
    /// [`Future`]: std::future::Future
    // TODO (evdokimovs): Remove it and use Updatable trait
    #[inline]
    pub fn when_all_updated(&self) -> LocalBoxFuture<'static, ()> {
        self.when_updated()
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

/// Component reponsible for the [`PeerConnection`] updating.
pub type PeerComponent = Component<PeerState, PeerConnection, GlobalCtx>;

#[watchers]
impl PeerComponent {
    /// Watcher for the [`PeerState::ice_candidates`] push update.
    ///
    /// Calls [`PeerConnection::add_ice_candidate`] with a pushed
    /// [`IceCandidate`].
    #[watch(self.state().ice_candidates.borrow().on_insert())]
    #[inline]
    async fn ice_candidate_insert_watcher(
        ctx: Rc<PeerConnection>,
        _: Rc<GlobalCtx>,
        _: Rc<PeerState>,
        candidate: IceCandidate,
    ) -> Result<(), Traced<PeerError>> {
        ctx.add_ice_candidate(
            candidate.candidate,
            candidate.sdp_m_line_index,
            candidate.sdp_mid,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())?;

        Ok(())
    }

    /// Watcher for the [`PeerState::remote_sdp_offer`] update.
    ///
    /// Calls [`PeerConnection::set_remote_answer`] with a new value if current
    /// [`NegotiationRole`] is [`NegotiationRole::Offerer`].
    ///
    /// Calls [`PeerConnection::set_remote_offer`] with a new value if current
    /// [`NegotiationRole`] is [`NegotiationRole::Answerer`].
    #[watch(self.state().remote_sdp_offer.subscribe())]
    async fn remote_sdp_offer_watcher(
        ctx: Rc<PeerConnection>,
        _: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        remote_sdp_answer: Guarded<Option<String>>,
    ) -> Result<(), Traced<PeerError>> {
        let (remote_sdp_answer, _guard) = remote_sdp_answer.into_parts();
        if let Some(remote_sdp_answer) = remote_sdp_answer {
            if let Some(role) = state.negotiation_role.get() {
                match role {
                    NegotiationRole::Offerer => {
                        ctx.set_remote_answer(remote_sdp_answer)
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
                        state.negotiation_state.set(NegotiationState::Stable);
                    }
                    NegotiationRole::Answerer(_) => {
                        ctx.set_remote_offer(remote_sdp_answer)
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Watcher for the [`PeerState::restart_ice`] update.
    ///
    /// Calls [`PeerConnection::restart_ice`] if new value is `true`.
    ///
    /// Resets [`PeerState::restart_ice`] to `false` if new value is `true`.
    #[watch(self.state().restart_ice.subscribe())]
    #[inline]
    async fn ice_restart_watcher(
        ctx: Rc<PeerConnection>,
        _: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        val: bool,
    ) -> Result<(), Traced<PeerError>> {
        if val {
            ctx.restart_ice();
            state.restart_ice.set(false);
        }

        Ok(())
    }

    #[watch(self.state().negotiation_state.subscribe().skip(1))]
    async fn negotiation_state_watcher(
        ctx: Rc<PeerConnection>,
        _: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        negotiation_state: NegotiationState,
    ) -> Result<(), Traced<PeerError>> {
        // TODO (evdokimovs): For more correctness we should wait for all
        //                    updates here.
        //                    But this kind of situation is unreachable atm.
        match negotiation_state {
            NegotiationState::Stable => {
                state.negotiation_role.set(None);
            }
            NegotiationState::WaitLocalSdp => {
                if let Some(negotiation_role) = state.negotiation_role.get() {
                    match negotiation_role {
                        NegotiationRole::Offerer => {
                            let sdp_offer = ctx
                                .peer
                                .create_offer()
                                .await
                                .map_err(tracerr::map_from_and_wrap!())?;
                            state.sdp_offer.update_offer_by_client(sdp_offer);
                        }
                        NegotiationRole::Answerer(_) => {
                            let sdp_answer = ctx
                                .peer
                                .create_answer()
                                .await
                                .map_err(tracerr::map_from_and_wrap!())?;
                            state.sdp_offer.update_offer_by_client(sdp_answer);
                        }
                    }
                }
            }
            _ => (),
        }

        Ok(())
    }

    #[watch(self.state().sdp_offer.on_approve().skip(1))]
    async fn sdp_offer_approve_watcher(
        _: Rc<PeerConnection>,
        _: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        _: (),
    ) -> Result<(), Traced<PeerError>> {
        if let Some(negotiation_role) = state.negotiation_role.get() {
            match negotiation_role {
                NegotiationRole::Offerer => {
                    state
                        .negotiation_state
                        .set(NegotiationState::WaitRemoteSdp);
                }
                NegotiationRole::Answerer(_) => {
                    state.negotiation_state.set(NegotiationState::Stable);
                }
            }
        }

        Ok(())
    }

    /// Watcher for the [`PeerState::senders`] insert update.
    ///
    /// Waits for [`ReceiverComponent`]s creation end.
    ///
    /// Waits for remote SDP offer apply if current [`NegotiationRole`] is
    /// [`NegotiationRole::Answerer`].
    ///
    /// Creates new [`SenderComponent`], creates new [`Connection`] with all
    /// [`SenderState::receivers`] by [`Connections::create_connection`] call,
    #[watch(self.state().senders.on_insert())]
    async fn sender_insert_watcher(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        val: Guarded<(TrackId, Rc<SenderState>)>,
    ) -> Result<(), Traced<PeerError>> {
        state.receivers.when_all_processed().await;
        if matches!(
            state.negotiation_role.get(),
            Some(NegotiationRole::Answerer(_))
        ) {
            state.remote_sdp_offer.when_all_processed().await;
        }

        let ((track_id, new_sender), _guard) = val.into_parts();
        for receiver in new_sender.receivers() {
            global_ctx.connections.create_connection(state.id, receiver);
        }
        let sndr = SenderBuilder {
            media_connections: &ctx.media_connections,
            track_id,
            caps: new_sender.media_type().clone().into(),
            mute_state: mute_state::Stable::from(new_sender.is_muted()),
            mid: new_sender.mid().clone(),
            media_exchange_state: media_exchange_state::Stable::from(
                !new_sender.is_enabled_individual(),
            ),
            required: new_sender.media_type().required(),
            send_constraints: ctx.send_constraints.clone(),
        }
        .build()
        .map_err(tracerr::map_from_and_wrap!())?;
        let component = spawn_component!(
            SenderComponent,
            new_sender,
            sndr,
            global_ctx.clone(),
        );
        ctx.media_connections.insert_sender(component);

        Ok(())
    }

    /// Watcher for the [`PeerState::receivers`] insert update.
    ///
    /// Creates new [`ReceiverComponent`], creates new [`Connection`] with a
    /// [`ReceiverState::sender_id`] by [`Connections::create_connection`] call,
    #[watch(self.state().receivers.on_insert())]
    async fn receiver_insert_watcher(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        val: Guarded<(TrackId, Rc<ReceiverState>)>,
    ) -> Result<(), Traced<PeerError>> {
        let ((track_id, new_receiver), _guard) = val.into_parts();
        global_ctx
            .connections
            .create_connection(state.id, new_receiver.sender_id());
        let recv = Receiver::new(
            &ctx.media_connections,
            track_id,
            new_receiver.media_type().clone().into(),
            new_receiver.sender_id().clone(),
            new_receiver.mid().clone(),
            new_receiver.enabled_general(),
            new_receiver.enabled_individual(),
        );
        let component = spawn_component!(
            ReceiverComponent,
            new_receiver,
            Rc::new(recv),
            global_ctx,
        );
        ctx.media_connections.insert_receiver(component);

        Ok(())
    }

    /// Watcher for the [`PeerState::sdp_offer`] updates.
    ///
    /// Sets [`PeerConnection`]'s SDP offer to the provided one and sends
    /// [`Command::MakeSdpOffer`] if [`Sdp`] is [`Sdp::Offer`] and
    /// [`NegotiationRole`] is [`NegotiationRole::Offerer`].
    ///
    /// Sets [`PeerConnection`]'s SDP answer to the provided one and sends
    /// [`Command::MakeSdpAnswer`] if [`Sdp`] is [`Sdp::Offer`] and
    /// [`NegotiationRole`] is [`NegotiationRole::Answerer`].
    ///
    /// Rollbacks [`PeerConnection`] to the stable state if [`Sdp`] is
    /// [`Sdp::Rollback`] and [`NegotiationRole`] is `Some`.
    #[watch(self.state().sdp_offer.on_new_local_sdp())]
    async fn sdp_offer_watcher(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        sdp_offer: Sdp,
    ) -> Result<(), Traced<PeerError>> {
        if let Some(role) = state.negotiation_role.get() {
            match (sdp_offer, role) {
                (Sdp::Offer(offer), NegotiationRole::Offerer) => {
                    ctx.peer
                        .set_offer(&offer)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    let mids = ctx
                        .get_mids()
                        .map_err(tracerr::map_from_and_wrap!())?;
                    global_ctx.rpc.send_command(Command::MakeSdpOffer {
                        peer_id: ctx.id(),
                        sdp_offer: offer,
                        transceivers_statuses: ctx.get_transceivers_statuses(),
                        mids,
                    });
                    state
                        .negotiation_state
                        .set(NegotiationState::WaitLocalSdpApprove);
                }
                (Sdp::Offer(offer), NegotiationRole::Answerer(_)) => {
                    ctx.peer
                        .set_answer(&offer)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    global_ctx.rpc.send_command(Command::MakeSdpAnswer {
                        peer_id: ctx.id(),
                        sdp_answer: offer,
                        transceivers_statuses: ctx.get_transceivers_statuses(),
                    });
                    state
                        .negotiation_state
                        .set(NegotiationState::WaitLocalSdpApprove);
                }
                (Sdp::Rollback(is_restart), _) => {
                    ctx.peer
                        .rollback()
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    if is_restart {
                        state
                            .negotiation_state
                            .set(NegotiationState::WaitLocalSdp);
                    } else {
                        state.negotiation_state.set(NegotiationState::Stable);
                    }
                }
            }
        }

        Ok(())
    }

    /// Watcher for the [`PeerState::negotiation_role`] updates.
    ///
    /// Waits for [`SenderComponent`]s/[`ReceiverComponent`]s creation/update,
    /// updates local `MediaStream` (if needed) and renegotiates
    /// [`PeerConnection`].
    #[watch(self.state().negotiation_role.subscribe())]
    async fn negotiation_role_watcher(
        ctx: Rc<PeerConnection>,
        _: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        new_negotiation_role: Option<NegotiationRole>,
    ) -> Result<(), Traced<PeerError>> {
        let _ = state.restart_ice.when_eq(false).await;
        if let Some(role) = new_negotiation_role {
            match role {
                NegotiationRole::Offerer => {
                    futures::future::join(
                        state.senders.when_all_processed(),
                        state.receivers.when_all_processed(),
                    )
                    .await;
                    state.when_all_updated().await;

                    state
                        .update_local_stream(&ctx)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;

                    ctx.media_connections.sync_receivers();
                    state.negotiation_state.set(NegotiationState::WaitLocalSdp);
                    ctx.media_connections.sync_receivers();
                }
                NegotiationRole::Answerer(remote_sdp_offer) => {
                    state.receivers.when_all_processed().await;
                    ctx.media_connections.sync_receivers();

                    state.set_remote_sdp_offer(remote_sdp_offer);

                    state.receivers.when_updated().await;
                    state.remote_sdp_offer.when_all_processed().await;
                    state.senders.when_all_processed().await;
                    state.senders.when_updated().await;

                    state
                        .update_local_stream(&ctx)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;

                    state.negotiation_state.set(NegotiationState::WaitLocalSdp);
                }
            }
        }

        Ok(())
    }
}
