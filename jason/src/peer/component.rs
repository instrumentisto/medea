use crate::utils::Component;

use super::PeerConnection;
use crate::{
    api::RoomCtx,
    peer::{
        media::{ReceiverState, SenderBuilder, SenderState},
        media_exchange_state, mute_state, LocalStreamUpdateCriteria, Receiver,
    },
};
use futures::stream;
use medea_client_api_proto::{
    Command, IceCandidate, IceServer, NegotiationRole, PeerId, TrackId,
};
use medea_reactive::{
    collections::ProgressableHashMap, Guarded, ObservableCell,
    ObservableHashMap, ObservableVec,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, Debug)]
enum NegotiationState {
    HaveRemote,
    HaveLocal,
    Stable,
}

pub struct PeerState {
    peer_id: PeerId,
    senders: RefCell<ProgressableHashMap<TrackId, Rc<SenderState>>>,
    receivers: RefCell<ProgressableHashMap<TrackId, Rc<ReceiverState>>>,
    ice_servers: Vec<IceServer>,
    force_relay: bool,
    negotiation_role: ObservableCell<Option<NegotiationRole>>,
    restart_ice: ObservableCell<bool>,
    ice_candidates: RefCell<ObservableVec<IceCandidate>>,
    remote_sdp_answer: ObservableCell<Option<String>>,
}

impl PeerState {
    pub fn new(
        senders: ProgressableHashMap<TrackId, Rc<SenderState>>,
        receivers: ProgressableHashMap<TrackId, Rc<ReceiverState>>,
        peer_id: PeerId,
        ice_servers: Vec<IceServer>,
        force_relay: bool,
        negotiation_role: Option<NegotiationRole>,
    ) -> Self {
        Self {
            senders: RefCell::new(senders),
            receivers: RefCell::new(receivers),
            peer_id,
            ice_servers,
            force_relay,
            negotiation_role: ObservableCell::new(negotiation_role),
            restart_ice: ObservableCell::new(false),
            ice_candidates: RefCell::new(ObservableVec::new()),
            remote_sdp_answer: ObservableCell::new(None),
        }
    }

    pub fn ice_servers(&self) -> &Vec<IceServer> {
        &self.ice_servers
    }

    pub fn force_relay(&self) -> bool {
        self.force_relay
    }

    pub fn insert_sender(&self, track_id: TrackId, sender: Rc<SenderState>) {
        self.senders.borrow_mut().insert(track_id, sender);
    }

    pub fn insert_receiver(
        &self,
        track_id: TrackId,
        receiver: Rc<ReceiverState>,
    ) {
        self.receivers.borrow_mut().insert(track_id, receiver);
    }

    pub fn get_sender(&self, track_id: TrackId) -> Option<Rc<SenderState>> {
        self.senders.borrow().get(&track_id).cloned()
    }

    pub fn get_receiver(&self, track_id: TrackId) -> Option<Rc<ReceiverState>> {
        self.receivers.borrow().get(&track_id).cloned()
    }

    pub fn set_negotiation_role(&self, negotiation_role: NegotiationRole) {
        self.negotiation_role.set(Some(negotiation_role));
    }

    pub fn restart_ice(&self) {
        self.restart_ice.set(true);
    }

    pub fn set_remote_sdp_answer(&self, new_remote_sdp_offer: String) {
        self.remote_sdp_answer.set(Some(new_remote_sdp_offer));
    }

    pub fn add_ice_candidate(&self, ice_candidate: IceCandidate) {
        self.ice_candidates.borrow_mut().push(ice_candidate);
    }
}

pub type PeerComponent = Component<PeerState, Rc<PeerConnection>, RoomCtx>;

impl PeerComponent {
    pub fn spawn(&self) {
        self.spawn_task(
            stream::select(
                self.state().senders.borrow().replay_on_insert(),
                self.state().senders.borrow().on_insert(),
            ),
            Self::handle_sender_insert,
        );
        self.spawn_task(
            stream::select(
                self.state().receivers.borrow().replay_on_insert(),
                self.state().receivers.borrow().on_insert(),
            ),
            Self::handle_receiver_insert,
        );
        self.spawn_task(
            self.state().negotiation_role.subscribe(),
            Self::handle_negotiation_role,
        );
        self.spawn_task(
            self.state().remote_sdp_answer.subscribe(),
            Self::handle_remove_sdp_answer,
        );
        self.spawn_task(
            self.state().ice_candidates.borrow().on_push(),
            Self::handle_ice_candidate_push,
        );
    }

    async fn handle_ice_candidate_push(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<PeerState>,
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

    async fn handle_remove_sdp_answer(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<PeerState>,
        remote_sdp_answer: Option<String>,
    ) {
        if let Some(remote_sdp_answer) = remote_sdp_answer {
            ctx.set_remote_answer(remote_sdp_answer).await.unwrap();
            state.negotiation_role.set(None);
        }
    }

    async fn handle_ice_restart(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<PeerState>,
        val: bool,
    ) {
        if val {
            ctx.restart_ice();
            state.restart_ice.set(false);
        }
    }

    async fn handle_sender_insert(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<PeerState>,
        val: Guarded<(TrackId, Rc<SenderState>)>,
    ) {
        state.receivers.borrow().when_all_processed().await;
        log::debug!("Sender inserted");
        let ((track_id, new_sender), _guard) = val.into_parts();
        // TODO: Unwrap here
        let sndr = SenderBuilder {
            media_connections: &ctx.media_connections,
            track_id,
            caps: new_sender.media_type().clone().into(),
            // TODO: this is temporary
            mute_state: mute_state::Stable::from(
                !new_sender.enabled_individual(),
            ),
            mid: new_sender.mid().clone(),
            media_exchange_state: media_exchange_state::Stable::from(
                !new_sender.enabled_individual(),
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

    async fn handle_receiver_insert(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<PeerState>,
        val: Guarded<(TrackId, Rc<ReceiverState>)>,
    ) {
        log::debug!("Receiver inserted");
        let ((track_id, new_receiver), _guard) = val.into_parts();
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

    async fn handle_negotiation_role(
        ctx: Rc<PeerConnection>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<PeerState>,
        new_negotiation_role: Option<NegotiationRole>,
    ) {
        state.restart_ice.when_eq(false).await;
        match new_negotiation_role {
            Some(NegotiationRole::Offerer) => {
                log::debug!("I'm offerer");
                futures::future::join(
                    state.senders.borrow().when_all_processed(),
                    state.receivers.borrow().when_all_processed(),
                )
                .await;
                ctx.media_connections.sync_receivers();
                let sdp_offer = ctx.peer.create_and_set_offer().await.unwrap();
                let mids = ctx.get_mids().unwrap();
                global_ctx.rpc.send_command(Command::MakeSdpOffer {
                    peer_id: ctx.id(),
                    sdp_offer,
                    transceivers_statuses: ctx.get_transceivers_statuses(),
                    mids,
                });
                ctx.media_connections.sync_receivers();
                ctx.update_local_stream(LocalStreamUpdateCriteria::all())
                    .await
                    .unwrap();
            }
            Some(NegotiationRole::Answerer(remote_sdp_offer)) => {
                log::debug!("I'm answerer");
                state.receivers.borrow().when_all_processed().await;
                ctx.media_connections.sync_receivers();
                // set offer, which will create transceivers and discover remote
                // tracks in receivers
                ctx.set_remote_offer(remote_sdp_offer).await.unwrap();
                state.senders.borrow().when_all_processed().await;
                let _ = ctx
                    .update_local_stream(LocalStreamUpdateCriteria::all())
                    .await;
                let sdp_answer =
                    ctx.peer.create_and_set_answer().await.unwrap();
                global_ctx.rpc.send_command(Command::MakeSdpAnswer {
                    peer_id: ctx.id(),
                    sdp_answer,
                    transceivers_statuses: ctx.get_transceivers_statuses(),
                });
                ctx.update_local_stream(LocalStreamUpdateCriteria::all())
                    .await
                    .unwrap();
                state.negotiation_role.set(None);
            }
            _ => (),
        }
    }
}
