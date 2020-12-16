//! Implementation of the [`PeerComponent`].

use std::{cell::RefCell, rc::Rc};

use futures::future::LocalBoxFuture;
use medea_client_api_proto::{
    Command, IceCandidate, IceServer, NegotiationRole, PeerId, TrackId,
};
use medea_macro::{watch, watchers};
use medea_reactive::{
    collections::ProgressableHashMap, Guarded, ObservableCell, ObservableVec,
    ProgressableCell,
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

/// State of the [`PeerComponent`].
pub struct PeerState {
    id: PeerId,
    senders: RefCell<ProgressableHashMap<TrackId, Rc<SenderState>>>,
    receivers: RefCell<ProgressableHashMap<TrackId, Rc<ReceiverState>>>,
    ice_servers: Vec<IceServer>,
    force_relay: bool,
    negotiation_role: ObservableCell<Option<NegotiationRole>>,
    sdp_offer: LocalSdp,
    remote_sdp_offer: ProgressableCell<Option<String>>,
    restart_ice: ObservableCell<bool>,
    ice_candidates: RefCell<ObservableVec<IceCandidate>>,
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
            senders: RefCell::new(senders),
            receivers: RefCell::new(receivers),
            ice_servers,
            force_relay,
            remote_sdp_offer: ProgressableCell::new(None),
            sdp_offer: LocalSdp::new(),
            negotiation_role: ObservableCell::new(negotiation_role),
            restart_ice: ObservableCell::new(false),
            ice_candidates: RefCell::new(ObservableVec::new()),
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
        self.senders.borrow_mut().insert(track_id, sender);
    }

    /// Inserts new [`ReceiverState`] into this [`PeerState`].
    #[inline]
    pub fn insert_receiver(
        &self,
        track_id: TrackId,
        receiver: Rc<ReceiverState>,
    ) {
        self.receivers.borrow_mut().insert(track_id, receiver);
    }

    /// Returns [`Rc`] to the [`SenderState`] with a provided [`TrackId`].
    #[inline]
    pub fn get_sender(&self, track_id: TrackId) -> Option<Rc<SenderState>> {
        self.senders.borrow().get(&track_id).cloned()
    }

    /// Returns [`Rc`] to the [`ReceiverState`] with a provided [`TrackId`].
    #[inline]
    pub fn get_receiver(&self, track_id: TrackId) -> Option<Rc<ReceiverState>> {
        self.receivers.borrow().get(&track_id).cloned()
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
        self.ice_candidates.borrow_mut().push(ice_candidate);
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

    /// Returns [`Future`] which will be resolved when all [`SenderState`]s
    /// updates will be applied.
    ///
    /// [`Future`]: std::future::Future
    fn when_all_senders_updated(&self) -> LocalBoxFuture<'static, ()> {
        let when_futs: Vec<_> = self
            .senders
            .borrow()
            .values()
            .map(|s| s.when_updated())
            .collect();
        let fut = futures::future::join_all(when_futs);
        Box::pin(async move {
            fut.await;
        })
    }

    /// Returns [`Future`] which will be resolved when all [`ReceiverState`]s
    /// updates will be applied.
    ///
    /// [`Future`]: std::future::Future
    fn when_all_receivers_updated(&self) -> LocalBoxFuture<'static, ()> {
        let when_futs: Vec<_> = self
            .receivers
            .borrow()
            .values()
            .map(|s| s.when_updated())
            .collect();
        let fut = futures::future::join_all(when_futs);
        Box::pin(async move {
            fut.await;
        })
    }

    /// Returns [`Future`] which will be resolved when all
    /// [`SenderState`]s/[`ReceiverState`]s updates will be applied.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_all_updated(&self) -> LocalBoxFuture<'static, ()> {
        let fut = futures::future::join(
            self.when_all_receivers_updated(),
            self.when_all_senders_updated(),
        );
        Box::pin(async move {
            fut.await;
        })
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
        let senders: Vec<_> = self
            .senders
            .borrow()
            .values()
            .filter(|s| s.is_local_stream_update_needed())
            .cloned()
            .collect();
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
    #[watch(self.state().ice_candidates.borrow().on_push())]
    #[inline]
    async fn ice_candidate_push_watcher(
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
                        state.negotiation_role.set(None);
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

    /// Watcher for the [`PeerState::senders`] insert update.
    ///
    /// Waits for [`ReceiverComponent`]s creation end.
    ///
    /// Waits for remote SDP offer apply if current [`NegotiationRole`] is
    /// [`NegotiationRole::Answerer`].
    ///
    /// Creates new [`SenderComponent`], creates new [`Connection`] with all
    /// [`SenderState::receivers`] by [`Connections::create_connection`] call,
    #[watch(self.state().senders.borrow().on_insert_with_replay())]
    async fn sender_insert_watcher(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        val: Guarded<(TrackId, Rc<SenderState>)>,
    ) -> Result<(), Traced<PeerError>> {
        state.receivers.borrow().when_all_processed().await;
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
    #[watch(self.state().receivers.borrow().on_insert_with_replay())]
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
                    state.sdp_offer.when_approved().await;
                    state.negotiation_role.set(None);
                }
                (Sdp::Rollback, _) => {
                    ctx.peer
                        .rollback()
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
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
                        state.senders.borrow().when_all_processed(),
                        state.receivers.borrow().when_all_processed(),
                    )
                    .await;
                    state.when_all_updated().await;

                    state
                        .update_local_stream(&ctx)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;

                    ctx.media_connections.sync_receivers();
                    let sdp_offer = ctx
                        .peer
                        .create_offer()
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    state.sdp_offer.update_offer(sdp_offer);
                    ctx.media_connections.sync_receivers();
                }
                NegotiationRole::Answerer(remote_sdp_offer) => {
                    state.receivers.borrow().when_all_processed().await;
                    ctx.media_connections.sync_receivers();

                    state.set_remote_sdp_offer(remote_sdp_offer);

                    state.when_all_receivers_updated().await;
                    state.remote_sdp_offer.when_all_processed().await;
                    state.senders.borrow().when_all_processed().await;
                    state.when_all_senders_updated().await;

                    state
                        .update_local_stream(&ctx)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;

                    let sdp_answer = ctx
                        .peer
                        .create_answer()
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    state.sdp_offer.update_offer(sdp_answer);
                }
            }
        }

        Ok(())
    }
}
