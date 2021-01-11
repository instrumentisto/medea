use std::rc::Rc;

use futures::StreamExt as _;
use medea_client_api_proto::{IceCandidate, NegotiationRole, TrackId};
use medea_macro::{watch, watchers};
use medea_reactive::Guarded;
use tracerr::Traced;

use crate::{
    peer::{
        component::{NegotiationState, SyncState},
        media::{receiver, sender},
        PeerError, PeerEvent,
    },
    utils::Updatable as _,
};

use super::{Component, PeerConnection, State};

#[watchers]
impl Component {
    /// Watcher for the [`State::ice_candidates`] push update.
    ///
    /// Calls [`PeerConnection::add_ice_candidate`] with a pushed
    /// [`IceCandidate`].
    #[watch(self.state().ice_candidates.on_add())]
    #[inline]
    async fn ice_candidate_push_watcher(
        peer: Rc<PeerConnection>,
        _: Rc<State>,
        candidate: IceCandidate,
    ) -> Result<(), Traced<PeerError>> {
        peer.add_ice_candidate(
            candidate.candidate,
            candidate.sdp_m_line_index,
            candidate.sdp_mid,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!())?;

        Ok(())
    }

    /// Watcher for the [`SyncState`] of this [`PeerComponent`].
    ///
    /// Will send intentions of the [`PeerComponent`] to the Media Server if
    /// [`SyncState`] is [`SyncState::Synced`].
    #[watch(self.state().sync_state.subscribe())]
    async fn sync_state_watcher(
        peer: Rc<PeerConnection>,
        _: Rc<State>,
        sync_state: SyncState,
    ) -> Result<(), Traced<PeerError>> {
        if let SyncState::Synced = sync_state {
            // peer.send_intentions();
        }

        Ok(())
    }

    /// Watcher for the [`State::remote_sdp_offer`] update.
    ///
    /// Calls [`PeerConnection::set_remote_answer`] with a new value if current
    /// [`NegotiationRole`] is [`NegotiationRole::Offerer`].
    ///
    /// Calls [`PeerConnection::set_remote_offer`] with a new value if current
    /// [`NegotiationRole`] is [`NegotiationRole::Answerer`].
    #[watch(self.state().remote_sdp_offer.subscribe())]
    async fn remote_sdp_offer_watcher(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        remote_sdp_answer: Guarded<Option<String>>,
    ) -> Result<(), Traced<PeerError>> {
        let (remote_sdp_answer, _guard) = remote_sdp_answer.into_parts();
        if let Some(remote_sdp_answer) = remote_sdp_answer {
            if let Some(role) = state.negotiation_role.get() {
                match role {
                    NegotiationRole::Offerer => {
                        peer.set_remote_answer(remote_sdp_answer)
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
                        state.negotiation_state.set(NegotiationState::Stable);
                        state.negotiation_role.set(None);
                    }
                    NegotiationRole::Answerer(_) => {
                        peer.set_remote_offer(remote_sdp_answer)
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Watcher for the [`State::restart_ice`] update.
    ///
    /// Calls [`PeerConnection::restart_ice`] if new value is `true`.
    ///
    /// Resets [`State::restart_ice`] to `false` if new value is `true`.
    #[watch(self.state().restart_ice.subscribe())]
    #[inline]
    async fn ice_restart_watcher(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        val: bool,
    ) -> Result<(), Traced<PeerError>> {
        if val {
            peer.restart_ice();
            state.restart_ice.set(false);
        }

        Ok(())
    }

    /// Watcher for the [`NegotiationState`] change.
    ///
    /// Resets [`NegotiationRole`] to `None` on [`NegotiationState::Stable`].
    ///
    /// Creates and sets local SDP offer on [`NegotiationState::WaitLocalSdp`].
    #[watch(self.state().negotiation_state.subscribe().skip(1))]
    async fn negotiation_state_watcher(
        ctx: Rc<PeerConnection>,
        state: Rc<State>,
        negotiation_state: NegotiationState,
    ) -> Result<(), Traced<PeerError>> {
        medea_reactive::join_all(vec![
            state.when_updated(),
            Box::new(state.senders.when_all_processed()),
            Box::new(state.receivers.when_all_processed()),
            Box::new(state.remote_sdp_offer.when_all_processed()),
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
    ///
    /// If current [`NegotiationRole`] is [`NegotiationRole::Offerer`] then
    /// [`NegotiationState`] will be transited to the
    /// [`NegotiationState::WaitRemoteSdp`].
    ///
    /// If current [`NegotiationRole`] is [`NegotiationRole::Answerer`] then
    /// [`NegotiationState`] will be transited to the
    /// [`NegotiationState::Stable`].
    #[watch(self.state().sdp_offer.on_approve().skip(1))]
    async fn sdp_offer_approve_watcher(
        _: Rc<PeerConnection>,
        state: Rc<State>,
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

    /// Watcher for the [`State::senders`] insert update.
    ///
    /// Waits for [`ReceiverComponent`]s creation end.
    ///
    /// Waits for remote SDP offer apply if current [`NegotiationRole`] is
    /// [`NegotiationRole::Answerer`].
    ///
    /// Creates new [`SenderComponent`], creates new [`Connection`] with all
    /// [`sender::State::receivers`] by [`Connections::create_connection`] call,
    #[watch(self.state().senders.on_insert())]
    async fn sender_insert_watcher(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        val: Guarded<(TrackId, Rc<sender::State>)>,
    ) -> Result<(), Traced<PeerError>> {
        state.when_all_receivers_processed().await;
        if matches!(
            state.negotiation_role.get(),
            Some(NegotiationRole::Answerer(_))
        ) {
            state.remote_sdp_offer.when_all_processed().await;
        }

        let ((_, new_sender), _guard) = val.into_parts();
        for receiver in new_sender.receivers() {
            peer.connections.create_connection(state.id, receiver);
        }
        peer.media_connections.insert_sender(
            sender::Component::new(
                new_sender,
                &peer.media_connections,
                peer.send_constraints.clone(),
                peer.track_events_sender.clone(),
            )
            .map_err(tracerr::map_from_and_wrap!())?,
        );

        Ok(())
    }

    /// Watcher for the [`State::receivers`] insert update.
    ///
    /// Creates new [`ReceiverComponent`], creates new [`Connection`] with a
    /// [`receiver::State::sender_id`] by [`Connections::create_connection`]
    /// call,
    #[watch(self.state().receivers.on_insert())]
    async fn receiver_insert_watcher(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        val: Guarded<(TrackId, Rc<receiver::State>)>,
    ) -> Result<(), Traced<PeerError>> {
        let ((_, new_receiver), _guard) = val.into_parts();
        peer.connections
            .create_connection(state.id, new_receiver.sender_id());
        peer.media_connections
            .insert_receiver(receiver::Component::new(
                new_receiver,
                &peer.media_connections,
                peer.track_events_sender.clone(),
                &peer.recv_constraints,
            ));

        Ok(())
    }

    /// Watcher for the [`State::sdp_offer`] updates.
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
    #[watch(self.state().sdp_offer.subscribe())]
    async fn sdp_offer_watcher(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        sdp_offer: Option<String>,
    ) -> Result<(), Traced<PeerError>> {
        state.sync_state.when_eq(SyncState::Synced).await.ok();
        if let Some(role) = state.negotiation_role.get() {
            if let Some(offer) = sdp_offer {
                if state.sdp_offer.is_rollback() {
                    peer.peer
                        .rollback()
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    if state.sdp_offer.is_restart_needed() {
                        state
                            .negotiation_state
                            .set(NegotiationState::WaitLocalSdp);
                    } else {
                        state.negotiation_state.set(NegotiationState::Stable);
                        state.negotiation_role.set(None);
                    }
                } else {
                    match role {
                        NegotiationRole::Offerer => {
                            peer.peer
                                .set_offer(&offer)
                                .await
                                .map_err(tracerr::map_from_and_wrap!())?;
                            let mids = peer
                                .get_mids()
                                .map_err(tracerr::map_from_and_wrap!())?;
                            peer.peer_events_sender
                                .unbounded_send(PeerEvent::NewSdpOffer {
                                    peer_id: peer.id(),
                                    sdp_offer: offer,
                                    transceivers_statuses: peer
                                        .get_transceivers_statuses(),
                                    mids,
                                })
                                .ok();
                            state
                                .negotiation_state
                                .set(NegotiationState::WaitLocalSdpApprove);
                        }
                        NegotiationRole::Answerer(_) => {
                            peer.peer
                                .set_answer(&offer)
                                .await
                                .map_err(tracerr::map_from_and_wrap!())?;
                            peer.peer_events_sender
                                .unbounded_send(PeerEvent::NewSdpAnswer {
                                    peer_id: peer.id(),
                                    sdp_answer: offer,
                                    transceivers_statuses: peer
                                        .get_transceivers_statuses(),
                                })
                                .ok();
                            state
                                .negotiation_state
                                .set(NegotiationState::WaitLocalSdpApprove);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Watcher for the [`State::negotiation_role`] updates.
    ///
    /// Waits for [`SenderComponent`]s/[`ReceiverComponent`]s creation/update,
    /// updates local `MediaStream` (if needed) and renegotiates
    /// [`PeerConnection`].
    #[watch(self.state().negotiation_role.subscribe())]
    async fn negotiation_role_watcher(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        new_negotiation_role: Option<NegotiationRole>,
    ) -> Result<(), Traced<PeerError>> {
        let _ = state.restart_ice.when_eq(false).await;
        if let Some(role) = new_negotiation_role {
            match role {
                NegotiationRole::Offerer => {
                    futures::future::join(
                        state.when_all_senders_processed(),
                        state.when_all_receivers_processed(),
                    )
                    .await;

                    state.senders.when_stabilized().await;
                    state.senders.when_updated().await;
                    state.receivers.when_stabilized().await;
                    state.receivers.when_updated().await;

                    state.when_updated().await;

                    state
                        .update_local_stream(&peer)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;

                    peer.media_connections.sync_receivers();
                    state.negotiation_state.set(NegotiationState::WaitLocalSdp);
                    peer.media_connections.sync_receivers();
                }
                NegotiationRole::Answerer(remote_sdp_offer) => {
                    state.when_all_receivers_processed().await;
                    peer.media_connections.sync_receivers();

                    state.set_remote_sdp_offer(remote_sdp_offer);

                    state.receivers.when_stabilized().await;
                    state.receivers.when_updated().await;
                    state.remote_sdp_offer.when_all_processed().await;
                    state.when_all_senders_processed().await;
                    state.senders.when_stabilized().await;
                    state.senders.when_updated().await;

                    state
                        .update_local_stream(&peer)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;

                    state.negotiation_state.set(NegotiationState::WaitLocalSdp);
                }
            }
        }

        Ok(())
    }
}
