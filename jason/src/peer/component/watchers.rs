//! Implementation of a [`Component`] watchers.

use std::rc::Rc;

use futures::{future, StreamExt as _};
use medea_client_api_proto::{IceCandidate, NegotiationRole, TrackId};
use medea_macro::watchers;
use medea_reactive::Guarded;
use tracerr::Traced;

use crate::{
    peer::{
        component::{NegotiationState, SyncState},
        media::{receiver, sender},
        PeerError, PeerEvent,
    },
    utils::{transpose_guarded, Updatable as _},
};

use super::{Component, PeerConnection, State};

#[watchers]
impl Component {
    /// Watcher for the [`State::ice_candidates`] push update.
    ///
    /// Calls [`PeerConnection::add_ice_candidate()`] with the pushed
    /// [`IceCandidate`].
    #[inline]
    #[watch(self.ice_candidates.on_add())]
    async fn ice_candidate_added(
        peer: Rc<PeerConnection>,
        _: Rc<State>,
        candidate: IceCandidate,
    ) -> Result<(), Traced<PeerError>> {
        log::error!("ice_candidate_added");
        let res = peer.add_ice_candidate(
            candidate.candidate,
            candidate.sdp_m_line_index,
            candidate.sdp_mid,
        )
        .await
        .map_err(tracerr::map_from_and_wrap!());
        log::error!("ice_candidate_added end");
        res
    }

    /// Watcher for the [`State::remote_sdp`] update.
    ///
    /// Calls [`PeerConnection::set_remote_answer()`] with a new value if the
    /// current [`NegotiationRole`] is an [`Offerer`].
    ///
    /// Calls [`PeerConnection::set_remote_offer()`] with a new value if the
    /// current [`NegotiationRole`] is an [`Answerer`].
    ///
    /// [`Answerer`]: NegotiationRole::Answerer
    /// [`Offerer`]: NegotiationRole::Offerer
    #[watch(self.remote_sdp.subscribe().filter_map(transpose_guarded))]
    async fn remote_sdp_changed(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        description: Guarded<String>,
    ) -> Result<(), Traced<PeerError>> {
        log::error!("remote_sdp_changed");
        let (description, _guard) = description.into_parts();
        if let Some(role) = state.negotiation_role.get() {
            match role {
                NegotiationRole::Offerer => {
                    log::debug!("REMOTE ANSWER");
                    peer.set_remote_answer(description)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    peer.media_connections.sync_receivers();
                    state.negotiation_state.set(NegotiationState::Stable);
                    state.negotiation_role.set(None);
                }
                NegotiationRole::Answerer(_) => {
                    log::debug!("REMOTE OFFER");
                    peer.set_remote_offer(description)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    peer.media_connections.sync_receivers();
                }
            }
        }
        log::error!("remote_sdp_changed end");
        Ok(())
    }

    /// Watcher for the [`State::senders`] remove update.
    ///
    /// Removes a [`sender::Component`] from the [`PeerConnection`].
    #[inline]
    #[watch(self.senders.on_remove())]
    async fn sender_removed(
        peer: Rc<PeerConnection>,
        _: Rc<State>,
        val: Guarded<(TrackId, Rc<sender::State>)>,
    ) -> Result<(), Traced<PeerError>> {
        log::error!("sender_removed");
        let ((track_id, _), _guard) = val.into_parts();
        peer.remove_track(track_id);
        log::error!("sender_removed END");
        Ok(())
    }

    /// Watcher for the [`State::receivers`] remove update.
    ///
    /// Removes a [`receiver::Component`] from the [`PeerConnection`].
    #[inline]
    #[watch(self.receivers.on_remove())]
    async fn receiver_removed(
        peer: Rc<PeerConnection>,
        _: Rc<State>,
        val: Guarded<(TrackId, Rc<receiver::State>)>,
    ) -> Result<(), Traced<PeerError>> {
        let ((track_id, _), _guard) = val.into_parts();
        peer.remove_track(track_id);
        Ok(())
    }

    /// Watcher for the [`State::senders`] insert update.
    ///
    /// Waits until [`receiver::Component`]s creation is finished.
    ///
    /// Waits for a remote SDP offer apply if the current [`NegotiationRole`] is
    /// an [`Answerer`].
    ///
    /// Creates a new [`sender::Component`], creates a new [`Connection`] with
    /// all [`sender::State::receivers`] by calling a
    /// [`Connections::create_connection()`][1].
    ///
    /// [`Answerer`]: NegotiationRole::Answerer
    /// [`Connection`]: crate::connection::Connection
    /// [1]: crate::connection::Connections::create_connection
    #[watch(self.senders.on_insert())]
    async fn sender_added(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        val: Guarded<(TrackId, Rc<sender::State>)>,
    ) -> Result<(), Traced<PeerError>> {
        let mut wait_futs = vec![state.when_all_receivers_processed().into()];
        if matches!(
            state.negotiation_role.get(),
            Some(NegotiationRole::Answerer(_))
        ) {
            wait_futs.push(state.remote_sdp.when_all_processed().into());
        }
        medea_reactive::when_all_processed(wait_futs).await;

        let ((_, new_sender), _guard) = val.into_parts();
        for receiver in new_sender.receivers() {
            peer.connections.create_connection(state.id, receiver);
        }
        let sender = match sender::Sender::new(
            &new_sender,
            &peer.media_connections,
            peer.send_constraints.clone(),
            peer.track_events_sender.clone(),
        ).await
        .map_err(tracerr::map_from_and_wrap!())
        {
            Ok(sender) => sender,
            Err(e) => {
                let _ = peer.peer_events_sender.unbounded_send(
                    PeerEvent::FailedLocalMedia {
                        error: e.clone().into(),
                    },
                );

                return Err(e);
            }
        };
        peer.media_connections
            .insert_sender(sender::Component::new(sender, new_sender));
        Ok(())
    }

    /// Watcher for the [`State::receivers`] insert update.
    ///
    /// Creates a new [`receiver::Component`], creates a new [`Connection`] with
    /// a [`receiver::State::sender_id`] by calling a
    /// [`Connections::create_connection()`][1].
    ///
    /// [`Connection`]: crate::connection::Connections
    /// [1]: crate::connection::Connections::create_connection
    #[watch(self.receivers.on_insert())]
    async fn receiver_added(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        val: Guarded<(TrackId, Rc<receiver::State>)>,
    ) -> Result<(), Traced<PeerError>> {
        log::error!("receiver_added");
        let ((_, receiver), _guard) = val.into_parts();
        peer.connections
            .create_connection(state.id, receiver.sender_id());
        peer.media_connections
            .insert_receiver(receiver::Component::new(
                Rc::new(receiver::Receiver::new(
                    &receiver,
                    &peer.media_connections,
                    peer.track_events_sender.clone(),
                    &peer.recv_constraints,
                ).await),
                receiver,
            ));
        log::error!("receiver_added END");
        Ok(())
    }

    /// Watcher for the [`State::local_sdp`] updates.
    ///
    /// Sets [`PeerConnection`]'s SDP offer to the provided one and sends
    /// a [`PeerEvent::NewSdpOffer`] if [`NegotiationRole`] is
    /// [`NegotiationRole::Offerer`].
    ///
    /// Sets [`PeerConnection`]'s SDP answer to the provided one and sends
    /// a [`PeerEvent::NewSdpAnswer`] if [`NegotiationRole`] is
    /// [`NegotiationRole::Answerer`].
    ///
    /// Rollbacks [`PeerConnection`] to a stable state if [`PeerConnection`] is
    /// marked for rollback and [`NegotiationRole`] is [`Some`].
    ///
    /// [`Answerer`]: NegotiationRole::Answerer
    /// [`Offerer`]: NegotiationRole::Offerer
    #[watch(self.local_sdp.subscribe().filter_map(future::ready))]
    async fn local_sdp_changed(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        sdp: String,
    ) -> Result<(), Traced<PeerError>> {
        log::error!("local_sdp_changed");
        let _ = state.sync_state.when_eq(SyncState::Synced).await;
        if let Some(role) = state.negotiation_role.get() {
            if state.local_sdp.is_rollback() {
                peer.peer
                    .rollback()
                    .await
                    .map_err(tracerr::map_from_and_wrap!())?;
                if state.local_sdp.is_restart_needed() {
                    state.negotiation_state.set(NegotiationState::WaitLocalSdp);
                } else {
                    state.negotiation_state.set(NegotiationState::Stable);
                    state.negotiation_role.set(None);
                }
            } else {
                match role {
                    NegotiationRole::Offerer => {
                        peer.peer
                            .set_offer(&sdp)
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
                        peer.media_connections.sync_receivers();
                        let mids = peer
                            .get_mids()
                            .map_err(tracerr::map_from_and_wrap!())?;
                        peer.peer_events_sender
                            .unbounded_send(PeerEvent::NewSdpOffer {
                                peer_id: peer.id(),
                                sdp_offer: sdp,
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
                            .set_answer(&sdp)
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
                        peer.media_connections.sync_receivers();
                        peer.peer_events_sender
                            .unbounded_send(PeerEvent::NewSdpAnswer {
                                peer_id: peer.id(),
                                sdp_answer: sdp,
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
        log::error!("local_sdp_changed END");
        Ok(())
    }

    /// Watcher for the SDP offer approving by server.
    ///
    /// If the current [`NegotiationRole`] is an [`NegotiationRole::Offerer`]
    /// then [`NegotiationState`] will transit to a [`WaitRemoteSdp`].
    ///
    /// If the current [`NegotiationRole`] is an [`NegotiationRole::Answerer`]
    /// then [`NegotiationState`] will transit to a [`Stable`].
    ///
    /// [`Offerer`]: NegotiationRole::Offerer
    /// [`Stable`]: NegotiationState::Stable
    /// [`WaitRemoteSdp`]: NegotiationState::WaitRemoteSdp
    #[watch(self.local_sdp.on_approve().skip(1))]
    async fn local_sdp_approved(
        _: Rc<PeerConnection>,
        state: Rc<State>,
        _: (),
    ) -> Result<(), Traced<PeerError>> {
        log::error!("local_sdp_approved");
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
        log::error!("local_sdp_approved END");
        Ok(())
    }

    /// Watcher for the [`NegotiationState`] change.
    ///
    /// Resets [`NegotiationRole`] to [`None`] on a
    /// [`NegotiationState::Stable`].
    ///
    /// Creates and sets local SDP offer on a
    /// [`NegotiationState::WaitLocalSdp`].
    #[watch(self.negotiation_state.subscribe().skip(1))]
    async fn negotiation_state_changed(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        negotiation_state: NegotiationState,
    ) -> Result<(), Traced<PeerError>> {
        medea_reactive::when_all_processed(vec![
            state.when_all_updated().into(),
            state.when_all_senders_processed().into(),
            state.when_all_receivers_processed().into(),
            state.remote_sdp.when_all_processed().into(),
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
                            if state.restart_ice.take() {
                                peer.restart_ice();
                            }
                            let sdp_offer = peer
                                .peer
                                .create_offer()
                                .await
                                .map_err(tracerr::map_from_and_wrap!())?;
                            state.local_sdp.unapproved_set(sdp_offer);
                        }
                        NegotiationRole::Answerer(_) => {
                            let sdp_answer = peer
                                .peer
                                .create_answer()
                                .await
                                .map_err(tracerr::map_from_and_wrap!())?;
                            state.local_sdp.unapproved_set(sdp_answer);
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }

    /// Watcher for the [`State::negotiation_role`] updates.
    ///
    /// Waits for [`sender::Component`]s' and [`receiver::Component`]s'
    /// creation/update, updates local `MediaStream` (if required) and
    /// renegotiates [`PeerConnection`].
    #[watch(self.negotiation_role.subscribe().filter_map(future::ready))]
    async fn negotiation_role_changed(
        _: Rc<PeerConnection>,
        state: Rc<State>,
        role: NegotiationRole,
    ) -> Result<(), Traced<PeerError>> {
        match role {
            NegotiationRole::Offerer => {
                medea_reactive::when_all_processed(vec![
                    state.when_all_senders_processed().into(),
                    state.when_all_receivers_processed().into(),
                ])
                .await;

                medea_reactive::when_all_processed(vec![
                    state.senders.when_stabilized().into(),
                    state.receivers.when_stabilized().into(),
                    state.when_all_updated().into(),
                ])
                .await;
            }
            NegotiationRole::Answerer(remote_sdp) => {
                state.when_all_receivers_processed().await;
                state.set_remote_sdp(remote_sdp);

                medea_reactive::when_all_processed(vec![
                    state.receivers.when_updated().into(),
                    state.senders.when_all_processed().into(),
                    state.remote_sdp.when_all_processed().into(),
                    state.senders.when_updated().into(),
                ])
                .await;

                medea_reactive::when_all_processed(vec![
                    state.senders.when_stabilized().into(),
                    state.receivers.when_stabilized().into(),
                ])
                .await;
            }
        }

        state.maybe_update_local_stream.set(true);
        let _ = state.maybe_update_local_stream.when_eq(false).await;

        state.negotiation_state.set(NegotiationState::WaitLocalSdp);

        Ok(())
    }

    /// Watcher for the [`State::sync_state`] updates.
    ///
    /// Sends [`PeerConnection`]'s connection state and ICE connection state to
    /// the server.
    #[inline]
    #[watch(self.sync_state.subscribe().skip(1))]
    async fn sync_state_changed(
        peer: Rc<PeerConnection>,
        _: Rc<State>,
        sync_state: SyncState,
    ) -> Result<(), Traced<PeerError>> {
        if let SyncState::Synced = sync_state {
            peer.send_current_connection_states();
        }
        Ok(())
    }

    /// Watcher for the [`State::maybe_update_local_stream`] `true` updates.
    ///
    /// Waits for [`State::senders`] update and calls
    /// [`State::update_local_stream()`].
    #[watch(
        self.maybe_update_local_stream.subscribe().filter(|v| future::ready(*v))
    )]
    async fn maybe_local_stream_update_needed(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        _: bool,
    ) -> Result<(), Traced<PeerError>> {
        state.senders.when_updated().await;
        let _ = state.update_local_stream(&peer).await;

        state.maybe_update_local_stream.set(false);
        Ok(())
    }
}
