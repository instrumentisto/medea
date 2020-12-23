use std::rc::Rc;

use futures::{future, StreamExt as _};
use medea_client_api_proto::{Command, IceCandidate, NegotiationRole, TrackId};
use medea_macro::{watch, watchers};
use medea_reactive::Guarded;
use tracerr::Traced;

use crate::{
    api::GlobalCtx,
    peer::{
        component::{NegotiationState, SyncState},
        media::{ReceiverState, SenderBuilder, SenderState},
        media_exchange_state, mute_state, PeerConnection, PeerError, PeerState,
        Receiver, ReceiverComponent, SenderComponent,
    },
    utils::Updatable as _,
};

use super::{local_sdp::Sdp, PeerComponent};

#[watchers]
impl PeerComponent {
    /// Watcher for the [`PeerState::ice_candidates`] push update.
    ///
    /// Calls [`PeerConnection::add_ice_candidate`] with a pushed
    /// [`IceCandidate`].
    #[watch(self.state().ice_candidates.on_add())]
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

    #[watch(self.global_ctx().rpc.on_connection_loss())]
    async fn connection_loss_watcher(
        _: Rc<PeerConnection>,
        _: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        _: (),
    ) -> Result<(), Traced<PeerError>> {
        state.sync_state.set(SyncState::Desynced);

        Ok(())
    }

    #[watch(self.global_ctx().rpc.on_reconnected())]
    async fn reconnect_watcher(
        _: Rc<PeerConnection>,
        _: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        _: (),
    ) -> Result<(), Traced<PeerError>> {
        state.sync_state.set(SyncState::Syncing);

        Ok(())
    }

    #[watch(self.state().sync_state.subscribe())]
    async fn sync_state_watcher(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<GlobalCtx>,
        _: Rc<PeerState>,
        sync_state: SyncState,
    ) -> Result<(), Traced<PeerError>> {
        if let SyncState::Synced = sync_state {
            if let Some(intent) = ctx.intentions() {
                global_ctx.rpc.send_command(intent);
            }
        }

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

    /// Watcher for the [`NegotiationState`] change.
    #[watch(self.state().negotiation_state.subscribe().skip(1))]
    async fn negotiation_state_watcher(
        ctx: Rc<PeerConnection>,
        _: Rc<GlobalCtx>,
        state: Rc<PeerState>,
        negotiation_state: NegotiationState,
    ) -> Result<(), Traced<PeerError>> {
        future::join_all(vec![
            state.when_updated(),
            state.senders.when_all_processed(),
            state.receivers.when_all_processed(),
        ])
        .await;

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

    /// Watcher for the SDP offer approving.
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
                    state.negotiation_role.set(None);
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
        state.sync_state.when_eq(SyncState::Synced).await.ok();
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
                        state.negotiation_role.set(None);
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
                    state.when_updated().await;

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
