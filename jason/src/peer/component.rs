use crate::utils::Component;

use super::PeerConnection;
use crate::peer::media::{SenderState, ReceiverState, SenderBuilder};
use std::rc::Rc;
use medea_reactive::ObservableHashMap;
use medea_client_api_proto::{TrackId, PeerId, IceServer};
use crate::peer::{media_exchange_state, mute_state, Receiver};
use futures::stream;

pub struct PeerState {
    peer_id: PeerId,
    senders: ObservableHashMap<TrackId, Rc<SenderState>>,
    receivers: ObservableHashMap<TrackId, Rc<ReceiverState>>,
    ice_servers: Vec<IceServer>,
    force_relay: bool,
}

impl PeerState {
    pub fn new(
        senders: ObservableHashMap<TrackId, Rc<SenderState>>,
        receivers: ObservableHashMap<TrackId, Rc<ReceiverState>>,
        peer_id: PeerId,
        ice_servers: Vec<IceServer>,
        force_relay: bool,
    ) -> Self {
        Self {
            senders,
            receivers,
            peer_id,
            ice_servers,
            force_relay
        }
    }

    pub fn ice_servers(&self) -> &Vec<IceServer> {
        &self.ice_servers
    }

    pub fn force_relay(&self) -> bool {
        self.force_relay
    }
}

pub type PeerComponent = Component<PeerState, Rc<PeerConnection>>;

impl PeerComponent {
    pub fn spawn(&self) {
        self.spawn_task(
            stream::select(
                self.state().senders.replay_on_insert(),
                self.state().senders.on_insert(),
            ),
            Self::handle_sender_insert,
        );
        self.spawn_task(
            stream::select(
                self.state().receivers.replay_on_insert(),
                self.state().receivers.on_insert(),
            ),
            Self::handle_receiver_insert,
        );
    }

    async fn handle_sender_insert(ctx: Rc<PeerConnection>, (track_id, new_sender): (TrackId, Rc<SenderState>)) {
        // TODO: Unwrap here
        let sndr = SenderBuilder {
            media_connections: &ctx.media_connections,
            track_id,
            caps: new_sender.media_type().clone().into(),
            // TODO: this is temporary
            mute_state: mute_state::Stable::from(new_sender.enabled_individual()),
            mid: new_sender.mid().clone(),
            media_exchange_state: media_exchange_state::Stable::from(new_sender.enabled_individual()),
            required: new_sender.media_type().required(),
            send_constraints: ctx.send_constraints.clone(),
        }
            .build()
            .unwrap();
        let component = Component::new_component(new_sender, sndr);
        component.spawn();
        ctx.media_connections.insert_sender(component);
    }

    async fn handle_receiver_insert(ctx: Rc<PeerConnection>, (track_id, new_receiver): (TrackId, Rc<ReceiverState>)) {
        let recv = Receiver::new(
            &ctx.media_connections,
            track_id,
            new_receiver.media_type().clone().into(),
            new_receiver.sender().clone(),
            new_receiver.mid().clone(),
            &ctx.recv_constraints,
        );
        let component =
            Component::new_component(new_receiver, Rc::new(recv));
        component.spawn();
        ctx.media_connections.insert_receiver(component);
    }
}