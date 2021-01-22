//! [`Component`] responsible for the [`PeerConnection`] updating.

use std::{cell::RefCell, rc::Rc};

use futures::{
    future, future::LocalBoxFuture, FutureExt, StreamExt as _, TryFutureExt,
};
use medea_client_api_proto as proto;
use medea_client_api_proto::{
    IceCandidate, IceServer, MediaSourceKind, NegotiationRole, PeerId as Id,
    TrackId,
};
use medea_macro::watchers;
use medea_reactive::{
    collections::ProgressableHashMap, AllProcessed, Guarded, ObservableCell,
    ObservableVec, ProgressableCell,
};
use tracerr::Traced;

use crate::{
    media::LocalTracksConstraints,
    peer::{
        local_sdp::LocalSdp,
        media::{receiver, sender},
        LocalStreamUpdateCriteria, PeerError, TransceiverSide,
    },
    utils::{component, transpose_guarded},
    MediaKind,
};

use super::{PeerConnection, PeerEvent};

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

/// State of the [`Component`].
#[derive(Debug)]
pub struct State {
    id: Id,
    senders: RefCell<ProgressableHashMap<TrackId, Rc<sender::State>>>,
    receivers: RefCell<ProgressableHashMap<TrackId, Rc<receiver::State>>>,
    ice_servers: Vec<IceServer>,
    force_relay: bool,
    negotiation_role: ObservableCell<Option<NegotiationRole>>,
    negotiation_state: ObservableCell<NegotiationState>,
    local_sdp: LocalSdp,
    remote_sdp: ProgressableCell<Option<String>>,
    restart_ice: ObservableCell<bool>,
    ice_candidates: RefCell<ObservableVec<IceCandidate>>,
}

impl State {
    /// Returns [`State`] with a provided data.
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
            senders: RefCell::new(ProgressableHashMap::new()),
            receivers: RefCell::new(ProgressableHashMap::new()),
            ice_servers,
            force_relay,
            local_sdp: LocalSdp::new(),
            remote_sdp: ProgressableCell::new(None),
            negotiation_role: ObservableCell::new(negotiation_role),
            negotiation_state: ObservableCell::new(NegotiationState::Stable),
            restart_ice: ObservableCell::new(false),
            ice_candidates: RefCell::new(ObservableVec::new()),
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

    /// Inserts new [`sender::State`] into this [`State`].
    #[inline]
    pub fn insert_sender(&self, track_id: TrackId, sender: Rc<sender::State>) {
        self.senders.borrow_mut().insert(track_id, sender);
    }

    /// Inserts new [`receiver::State`] into this [`State`].
    #[inline]
    pub fn insert_receiver(
        &self,
        track_id: TrackId,
        receiver: Rc<receiver::State>,
    ) {
        self.receivers.borrow_mut().insert(track_id, receiver);
    }

    /// Returns [`Rc`] to the [`sender::State`] with the provided [`TrackId`].
    #[inline]
    #[must_use]
    pub fn get_sender(&self, track_id: TrackId) -> Option<Rc<sender::State>> {
        self.senders.borrow().get(&track_id).cloned()
    }

    /// Returns [`Rc`] to the [`receiver::State`] with the provided [`TrackId`].
    #[inline]
    #[must_use]
    pub fn get_receiver(
        &self,
        track_id: TrackId,
    ) -> Option<Rc<receiver::State>> {
        self.receivers.borrow().get(&track_id).cloned()
    }

    /// Sets [`NegotiationRole`] of this [`State`] to the provided one.
    #[inline]
    pub async fn set_negotiation_role(
        &self,
        negotiation_role: NegotiationRole,
    ) {
        self.negotiation_role.when(Option::is_none).await.ok();
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
        self.ice_candidates.borrow_mut().push(ice_candidate);
    }

    /// Marks current [`LocalSdp`] as approved by server.
    #[inline]
    pub fn apply_local_sdp(&self, sdp: String) {
        self.local_sdp.approved_set(sdp);
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

    /// Returns [`Future`] which will be resolved when gUM/gDM request for the
    /// provided [`MediaKind`]/[`MediaSourceKind`] will be resolved.
    ///
    /// [`Result`] returned by this [`Future`] will be the same as result of the
    /// gUM/gDM request.
    ///
    /// Returns last known gUM/gDM request's [`Result`], if currently no gUM/gDM
    /// requests are running for the provided [`MediaKind`]/[`MediaSourceKind`].
    ///
    /// If provided [`None`] [`MediaSourceKind`] then result will be for all
    /// [`MediaSourceKind`]s.
    ///
    /// [`Future`]: std::future::Future
    pub fn local_stream_update_result(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> LocalBoxFuture<'static, Result<(), Traced<PeerError>>> {
        Box::pin(
            future::try_join_all(self.senders.borrow().values().filter_map(
                |s| {
                    if s.media_kind() == kind
                        && source_kind.map_or(true, |k| k == s.source_kind())
                    {
                        Some(
                            s.local_stream_update_result()
                                .map_err(tracerr::map_from_and_wrap!()),
                        )
                    } else {
                        None
                    }
                },
            ))
            .map(|r| r.map(|_| ())),
        )
    }

    /// Returns [`Future`] resolving when all [`sender::State`]'s updates will
    /// be applied.
    ///
    /// [`Future`]: std::future::Future
    fn when_all_senders_updated(&self) -> AllProcessed<'static> {
        let when_futs: Vec<_> = self
            .senders
            .borrow()
            .values()
            .map(|s| s.when_updated().into())
            .collect();
        medea_reactive::when_all_processed(when_futs)
    }

    /// Returns [`Future`] resolving when all [`receiver::State`]'s updates will
    /// be applied.
    ///
    /// [`Future`]: std::future::Future
    fn when_all_receivers_updated(&self) -> AllProcessed<'static> {
        let when_futs: Vec<_> = self
            .receivers
            .borrow()
            .values()
            .map(|s| s.when_updated().into())
            .collect();
        medea_reactive::when_all_processed(when_futs)
    }

    /// Returns [`Future`] which will be resolved when all [`State::receivers`]
    /// will be stabilized meaning that all [`ReceiverComponent`]s won't contain
    /// any pending state change transitions.
    fn when_all_receivers_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        let when_futs: Vec<_> = self
            .receivers
            .borrow()
            .values()
            .map(|s| s.when_stabilized())
            .collect();

        Box::pin(futures::future::join_all(when_futs).map(|_| ()))
    }

    /// Returns [`Future`] which will be resolved when all [`State::senders`]
    /// will be stabilized meaning that all [`SenderComponent`]s won't contain
    /// any pending state change transitions.
    fn when_all_senders_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        let when_futs: Vec<_> = self
            .senders
            .borrow()
            .values()
            .map(|s| s.when_stabilized())
            .collect();

        Box::pin(futures::future::join_all(when_futs).map(|_| ()))
    }

    /// Returns [`Future`] resolving when all [`sender::State`]'s and
    /// [`receiver::State`]'s updates will be applied.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_all_updated(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![
            self.when_all_receivers_updated().into(),
            self.when_all_senders_updated().into(),
        ])
    }

    /// Updates local `MediaStream` based on
    /// [`sender::State::is_local_stream_update_needed`].
    ///
    /// Resets [`sender::State`] local stream update when it's updated.
    async fn update_local_stream(
        &self,
        peer: &Rc<PeerConnection>,
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
        let res = peer
            .update_local_stream(criteria)
            .await
            .map_err(tracerr::map_from_and_wrap!())
            .map(|_| ());
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
                self.senders.borrow_mut().insert(
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
                self.receivers.borrow_mut().insert(
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

    /// Returns [`Future`] resolving when all [`State::senders`]' inserts and
    /// removes will be processed.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    fn when_all_senders_processed(&self) -> AllProcessed<'static> {
        self.senders.borrow().when_all_processed()
    }

    /// Returns [`Future`] resolving when all [`State::receivers`]' inserts and
    /// removes will be processed.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    fn when_all_receivers_processed(&self) -> AllProcessed<'static> {
        self.receivers.borrow().when_all_processed()
    }

    /// Patches [`sender::State`] or [`receiver::State`] with the provided
    /// [`proto::TrackPatchEvent`].
    pub fn patch_track(&self, track_patch: &proto::TrackPatchEvent) {
        if let Some(sender) = self.get_sender(track_patch.id) {
            sender.update(track_patch);
        } else if let Some(receiver) = self.get_receiver(track_patch.id) {
            receiver.update(track_patch);
        }
    }
}

/// Component responsible for the [`PeerConnection`] updating.
pub type Component = component::Component<State, PeerConnection>;

#[watchers]
impl Component {
    /// Watcher for the [`State::ice_candidates`] push update.
    ///
    /// Calls [`PeerConnection::add_ice_candidate()`] with the pushed
    /// [`IceCandidate`].
    #[inline]
    #[watch(self.ice_candidates.borrow().on_push())]
    async fn ice_candidate_added(
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
        let (description, _guard) = description.into_parts();
        if let Some(role) = state.negotiation_role.get() {
            match role {
                NegotiationRole::Offerer => {
                    peer.set_remote_answer(description)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    state.negotiation_state.set(NegotiationState::Stable);
                    state.negotiation_role.set(None);
                }
                NegotiationRole::Answerer(_) => {
                    peer.set_remote_offer(description)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                }
            }
        }
        Ok(())
    }

    /// Watcher for the [`State::restart_ice`] update.
    ///
    /// Calls [`PeerConnection::restart_ice()`] if new value is `true`.
    ///
    /// Resets [`State::restart_ice`] to `false` if new value is `true`.
    #[inline]
    #[watch(self.restart_ice.subscribe())]
    async fn ice_restart_scheduled(
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

    /// Watcher for the [`State::senders`] insert update.
    ///
    /// Waits until [`ReceiverComponent`]s creation id finished.
    ///
    /// Waits for remote SDP offer apply if the current [`NegotiationRole`] is
    /// an [`Answerer`].
    ///
    /// Creates new [`SenderComponent`], creates new [`Connection`] with all
    /// [`sender::State::receivers`] by calling
    /// [`Connections::create_connection()`].
    ///
    /// [`Answerer`]: NegotiationRole::Answerer
    #[watch(self.senders.borrow().on_insert_with_replay())]
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
        )
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
    /// Creates new [`ReceiverComponent`], creates new [`Connection`] with a
    /// [`receiver::State::sender_id`] by calling
    /// [`Connections::create_connection()`].
    #[watch(self.receivers.borrow().on_insert_with_replay())]
    async fn receiver_added(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        val: Guarded<(TrackId, Rc<receiver::State>)>,
    ) -> Result<(), Traced<PeerError>> {
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
                )),
                receiver,
            ));
        Ok(())
    }

    /// Watcher for the [`State::local_sdp`] updates.
    ///
    /// Sets [`PeerConnection`]'s SDP offer to the provided one and sends
    /// [`Command::MakeSdpOffer`] if [`Sdp`] is [`Sdp::Offer`] and
    /// [`NegotiationRole`] is [`Offerer`].
    ///
    /// Sets [`PeerConnection`]'s SDP answer to the provided one and sends
    /// [`Command::MakeSdpAnswer`] if [`Sdp`] is [`Sdp::Offer`] and
    /// [`NegotiationRole`] is [`Answerer`].
    ///
    /// Rollbacks [`PeerConnection`] to the stable state if [`Sdp`] is
    /// [`Sdp::Rollback`] and [`NegotiationRole`] is [`Some`].
    ///
    /// [`Answerer`]: NegotiationRole::Answerer
    /// [`Offerer`]: NegotiationRole::Offerer
    #[watch(self.local_sdp.subscribe().filter_map(future::ready))]
    async fn local_sdp_changed(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        sdp: String,
    ) -> Result<(), Traced<PeerError>> {
        if let Some(role) = state.negotiation_role.get() {
            if state.local_sdp.is_rollback() {
                peer.peer
                    .rollback()
                    .await
                    .map_err(tracerr::map_from_and_wrap!())?;
                state.negotiation_state.set(NegotiationState::Stable);
                state.negotiation_role.set(None);
            } else {
                match role {
                    NegotiationRole::Offerer => {
                        peer.peer
                            .set_offer(&sdp)
                            .await
                            .map_err(tracerr::map_from_and_wrap!())?;
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
    #[watch(self.local_sdp.on_approve().skip(1))]
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

    /// Watcher for the [`NegotiationState`] change.
    ///
    /// Resets [`NegotiationRole`] to `None` on [`NegotiationState::Stable`].
    ///
    /// Creates and sets local SDP offer on [`NegotiationState::WaitLocalSdp`].
    #[watch(self.negotiation_state.subscribe().skip(1))]
    async fn negotiation_state_watcher(
        ctx: Rc<PeerConnection>,
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
                            let sdp_offer = ctx
                                .peer
                                .create_offer()
                                .await
                                .map_err(tracerr::map_from_and_wrap!())?;
                            state.local_sdp.unapproved_set(sdp_offer);
                        }
                        NegotiationRole::Answerer(_) => {
                            let sdp_answer = ctx
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
    /// Waits for [`SenderComponent`]s' and [`ReceiverComponent`]s'
    /// creation/update, updates local `MediaStream` (if required) and
    /// renegotiates [`PeerConnection`].
    #[watch(self.negotiation_role.subscribe().filter_map(future::ready))]
    async fn negotiation_role_changed(
        peer: Rc<PeerConnection>,
        state: Rc<State>,
        role: NegotiationRole,
    ) -> Result<(), Traced<PeerError>> {
        let _ = state.restart_ice.when_eq(false).await;
        match role {
            NegotiationRole::Offerer => {
                futures::future::join(
                    state.when_all_senders_processed(),
                    state.when_all_receivers_processed(),
                )
                .await;
                state.when_all_senders_stabilized().await;
                state.when_all_receivers_stabilized().await;
                state.when_all_updated().await;

                state
                    .update_local_stream(&peer)
                    .await
                    .map_err(tracerr::map_from_and_wrap!())?;

                peer.media_connections.sync_receivers();
                state.negotiation_state.set(NegotiationState::WaitLocalSdp);
                peer.media_connections.sync_receivers();
            }
            NegotiationRole::Answerer(remote_sdp) => {
                state.when_all_receivers_processed().await;
                peer.media_connections.sync_receivers();

                state.set_remote_sdp(remote_sdp);

                medea_reactive::when_all_processed(vec![
                    state.when_all_receivers_updated().into(),
                    state.when_all_senders_processed().into(),
                    state.remote_sdp.when_all_processed().into(),
                    state.when_all_senders_updated().into(),
                ])
                .await;
                state.when_all_senders_stabilized().await;
                state.when_all_receivers_stabilized().await;

                state
                    .update_local_stream(&peer)
                    .await
                    .map_err(tracerr::map_from_and_wrap!())?;

                state.negotiation_state.set(NegotiationState::WaitLocalSdp);
            }
        }
        Ok(())
    }
}

#[cfg(feature = "mockable")]
impl State {
    /// Waits for [`State::remote_sdp`] change to be applied.
    #[inline]
    pub async fn when_remote_sdp_processed(&self) {
        self.remote_sdp.when_all_processed().await;
    }

    /// Resets [`NegotiationRole`] of this [`State`] to [`None`].
    #[inline]
    pub fn reset_negotiation_role(&self) {
        self.negotiation_state.set(NegotiationState::Stable);
        self.negotiation_role.set(None);
    }

    /// Returns [`Future`] which will be resolved when local SDP approve will be
    /// needed.
    #[inline]
    pub fn when_local_sdp_approve_needed(
        &self,
    ) -> impl future::Future<Output = ()> {
        self.negotiation_state
            .when_eq(NegotiationState::WaitLocalSdpApprove)
            .map(|_| ())
    }

    /// Stabilize all [`receiver::State`] from this [`State`].
    #[inline]
    pub fn stabilize_all(&self) {
        self.receivers.borrow().values().for_each(|r| {
            r.stabilize();
        });
    }

    /// Waits until [`State::local_sdp`] will be resolved and returns its new
    /// value.
    #[inline]
    pub async fn when_local_sdp_updated(&self) -> Option<String> {
        use futures::StreamExt as _;

        self.local_sdp.subscribe().skip(1).next().await.unwrap()
    }

    /// Waits until all [`State::senders`]' and [`State::receivers`]' inserts
    /// will be processed.
    #[inline]
    pub async fn when_all_tracks_created(&self) {
        medea_reactive::when_all_processed(vec![
            self.senders.borrow().when_insert_processed().into(),
            self.receivers.borrow().when_insert_processed().into(),
        ])
        .await;
    }
}
