use std::{cell::RefCell, rc::Rc};

use futures::{future::LocalBoxFuture, stream};
use medea_client_api_proto::{
    Command, IceCandidate, IceServer, NegotiationRole, PeerId, TrackId,
};
use medea_reactive::{
    collections::ProgressableHashMap, Guarded, ObservableCell, ObservableVec,
    ProgressableCell,
};

use crate::{
    api::RoomCtx,
    peer::{
        media::{ReceiverState, SenderBuilder, SenderState},
        media_exchange_state, mute_state, LocalStreamUpdateCriteria, Receiver,
    },
    utils::Component,
};

use super::PeerConnection;

pub struct PeerState {
    id: PeerId,
    senders: ProgressableHashMap<TrackId, Rc<SenderState>>,
    receivers: ProgressableHashMap<TrackId, Rc<ReceiverState>>,
    ice_servers: Vec<IceServer>,
    force_relay: bool,
    negotiation_role: ObservableCell<Option<NegotiationRole>>,
    remote_sdp_offer: ProgressableCell<Option<String>>,
    restart_ice: ObservableCell<bool>,
    ice_candidates: RefCell<ObservableVec<IceCandidate>>,
}

impl PeerState {
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
            senders,
            receivers,
            ice_servers,
            force_relay,
            remote_sdp_offer: ProgressableCell::new(None),
            negotiation_role: ObservableCell::new(negotiation_role),
            restart_ice: ObservableCell::new(false),
            ice_candidates: RefCell::new(ObservableVec::new()),
        }
    }

    pub fn ice_servers(&self) -> &Vec<IceServer> {
        &self.ice_servers
    }

    pub fn force_relay(&self) -> bool {
        self.force_relay
    }

    pub fn insert_sender(&self, track_id: TrackId, sender: Rc<SenderState>) {
        self.senders.insert(track_id, sender);
    }

    pub fn insert_receiver(
        &self,
        track_id: TrackId,
        receiver: Rc<ReceiverState>,
    ) {
        self.receivers.insert(track_id, receiver);
    }

    pub fn get_sender(&self, track_id: TrackId) -> Option<Rc<SenderState>> {
        self.senders.get(&track_id, |s| s.clone())
    }

    pub fn get_receiver(&self, track_id: TrackId) -> Option<Rc<ReceiverState>> {
        self.receivers.get(&track_id, |s| s.clone())
    }

    pub fn set_negotiation_role(&self, negotiation_role: NegotiationRole) {
        self.negotiation_role.set(Some(negotiation_role));
    }

    pub fn restart_ice(&self) {
        self.restart_ice.set(true);
    }

    pub fn set_remote_sdp_offer(&self, new_remote_sdp_offer: String) {
        self.remote_sdp_offer.set(Some(new_remote_sdp_offer));
    }

    pub fn add_ice_candidate(&self, ice_candidate: IceCandidate) {
        self.ice_candidates.borrow_mut().push(ice_candidate);
    }

    fn when_all_senders_updated(&self) -> LocalBoxFuture<'static, ()> {
        let when_futs: Vec<_> = self.senders.map_values(|s| s.when_updated());
        let fut = futures::future::join_all(when_futs);
        Box::pin(async move {
            fut.await;
        })
    }

    fn when_all_receivers_updated(&self) -> LocalBoxFuture<'static, ()> {
        let when_futs: Vec<_> = self.receivers.map_values(|s| s.when_updated());
        let fut = futures::future::join_all(when_futs);
        Box::pin(async move {
            fut.await;
        })
    }

    fn when_all_updated(&self) -> LocalBoxFuture<'static, ()> {
        let fut = futures::future::join(
            self.when_all_receivers_updated(),
            self.when_all_senders_updated(),
        );
        Box::pin(async move {
            fut.await;
        })
    }

    async fn update_local_stream(&self, ctx: &Rc<PeerConnection>) {
        let mut criteria = LocalStreamUpdateCriteria::empty();
        let senders: Vec<_> = self.senders.filter_map_values(|s| {
            if s.is_local_stream_update_needed() {
                Some(s.clone())
            } else {
                None
            }
        });
        for s in &senders {
            criteria.add(s.media_kind(), s.media_source());
        }
        ctx.update_local_stream(criteria).await.unwrap();
        for s in senders {
            s.local_stream_updated();
        }
    }
}

pub type PeerComponent = Component<PeerState, Rc<PeerConnection>, RoomCtx>;

impl PeerComponent {
    pub fn spawn(&self) {
        self.spawn_observer(
            stream::select(
                self.state().senders.replay_on_insert(),
                self.state().senders.on_insert(),
            ),
            Self::observe_sender_insert,
        );
        self.spawn_observer(
            stream::select(
                self.state().receivers.replay_on_insert(),
                self.state().receivers.on_insert(),
            ),
            Self::observe_receiver_insert,
        );
        self.spawn_observer(
            self.state().negotiation_role.subscribe(),
            Self::observe_negotiation_role,
        );
        self.spawn_observer(
            self.state().remote_sdp_offer.subscribe(),
            Self::observe_remote_sdp_offer,
        );
        self.spawn_observer(
            self.state().ice_candidates.borrow().on_push(),
            Self::observe_ice_candidate_push,
        );
        self.spawn_observer(
            self.state().restart_ice.subscribe(),
            Self::observe_ice_restart,
        );
    }

    async fn observe_ice_candidate_push(
        ctx: Rc<PeerConnection>,
        _: Rc<RoomCtx>,
        _: Rc<PeerState>,
        candidate: IceCandidate,
    ) {
        ctx.add_ice_candidate(
            candidate.candidate,
            candidate.sdp_m_line_index,
            candidate.sdp_mid,
        )
        .await
        .unwrap();
    }

    async fn observe_remote_sdp_offer(
        ctx: Rc<PeerConnection>,
        _: Rc<RoomCtx>,
        state: Rc<PeerState>,
        remote_sdp_answer: Guarded<Option<String>>,
    ) {
        let (remote_sdp_answer, _guard) = remote_sdp_answer.into_parts();
        if let Some(remote_sdp_answer) = remote_sdp_answer {
            if matches!(
                state.negotiation_role.get(),
                Some(NegotiationRole::Offerer)
            ) {
                ctx.set_remote_answer(remote_sdp_answer).await.unwrap();
                state.negotiation_role.set(None);
            } else if matches!(
                state.negotiation_role.get(),
                Some(NegotiationRole::Answerer(_))
            ) {
                ctx.set_remote_offer(remote_sdp_answer).await.unwrap();
            }
        }
    }

    async fn observe_ice_restart(
        ctx: Rc<PeerConnection>,
        _: Rc<RoomCtx>,
        state: Rc<PeerState>,
        val: bool,
    ) {
        if val {
            ctx.restart_ice();
            state.restart_ice.set(false);
        }
    }

    async fn observe_sender_insert(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<PeerState>,
        val: Guarded<(TrackId, Rc<SenderState>)>,
    ) {
        let fut = state.receivers.when_all_processed();
        fut.await;
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
        .unwrap();
        let component =
            Component::new_component(new_sender, sndr, global_ctx.clone());
        component.spawn();
        ctx.media_connections.insert_sender(component);
    }

    async fn observe_receiver_insert(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<PeerState>,
        val: Guarded<(TrackId, Rc<ReceiverState>)>,
    ) {
        let ((track_id, new_receiver), _guard) = val.into_parts();
        global_ctx
            .connections
            .create_connection(state.id, new_receiver.sender());
        let recv = Receiver::new(
            &ctx.media_connections,
            track_id,
            new_receiver.media_type().clone().into(),
            new_receiver.sender().clone(),
            new_receiver.mid().clone(),
            &ctx.recv_constraints,
        );
        let component = Component::new_component(
            new_receiver,
            Rc::new(recv),
            global_ctx.clone(),
        );
        component.spawn();
        ctx.media_connections.insert_receiver(component);
    }

    async fn observe_negotiation_role(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<PeerState>,
        new_negotiation_role: Option<NegotiationRole>,
    ) {
        state.restart_ice.when_eq(false).await.unwrap();
        if let Some(role) = new_negotiation_role {
            match role {
                NegotiationRole::Offerer => {
                    futures::future::join(
                        state.senders.when_all_processed(),
                        state.receivers.when_all_processed(),
                    )
                    .await;
                    state.when_all_updated().await;

                    state.update_local_stream(&ctx).await;

                    ctx.media_connections.sync_receivers();
                    let sdp_offer =
                        ctx.peer.create_and_set_offer().await.unwrap();
                    let mids = ctx.get_mids().unwrap();
                    global_ctx.rpc.send_command(Command::MakeSdpOffer {
                        peer_id: ctx.id(),
                        sdp_offer,
                        transceivers_statuses: ctx.get_transceivers_statuses(),
                        mids,
                    });
                    ctx.media_connections.sync_receivers();
                }
                NegotiationRole::Answerer(remote_sdp_offer) => {
                    state.receivers.when_all_processed().await;
                    ctx.media_connections.sync_receivers();

                    state.set_remote_sdp_offer(remote_sdp_offer);

                    state.when_all_receivers_updated().await;
                    state.remote_sdp_offer.when_all_processed().await;
                    state.senders.when_all_processed().await;
                    state.when_all_senders_updated().await;

                    state.update_local_stream(&ctx).await;

                    let sdp_answer =
                        ctx.peer.create_and_set_answer().await.unwrap();
                    global_ctx.rpc.send_command(Command::MakeSdpAnswer {
                        peer_id: ctx.id(),
                        sdp_answer,
                        transceivers_statuses: ctx.get_transceivers_statuses(),
                    });
                    state.negotiation_role.set(None);
                }
            }
        }
    }
}
