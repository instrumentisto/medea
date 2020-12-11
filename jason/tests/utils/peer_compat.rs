use std::{cell::RefCell, ops::Deref, rc::Rc};

use futures::{
    channel::mpsc, future, stream, stream::LocalBoxStream, StreamExt as _,
};
use medea_client_api_proto as proto;
use medea_client_api_proto::{
    Command, Direction, IceServer, NegotiationRole, PeerId,
};
use medea_jason::{
    api::{Connections, Ctx},
    media::{LocalTracksConstraints, MediaManager, RecvConstraints},
    peer::{
        MediaConnectionsError, PeerComponent, PeerConnection, PeerError,
        PeerEvent, PeerState, ReceiverState, SenderState,
    },
    rpc::MockRpcSession,
    spawn_component,
};
use medea_reactive::ProgressableHashMap;
use tracerr::Traced;

pub struct PeerConnectionCompatibility {
    component: PeerComponent,
    command_rx: RefCell<LocalBoxStream<'static, Command>>,
    send_constraints: LocalTracksConstraints,
    recv_constraints: RecvConstraints,
}

impl PeerConnectionCompatibility {
    pub fn new(
        id: PeerId,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        ice_servers: Vec<IceServer>,
        media_manager: Rc<MediaManager>,
        is_force_relayed: bool,
        send_constraints: LocalTracksConstraints,
        recv_constraints: RecvConstraints,
    ) -> Result<Self, Traced<PeerError>> {
        let peer = PeerConnection::new(
            id,
            peer_events_sender,
            ice_servers.clone(),
            media_manager,
            is_force_relayed,
            send_constraints.clone(),
        )?;
        let state = PeerState::new(
            id,
            ProgressableHashMap::new(),
            ProgressableHashMap::new(),
            ice_servers,
            is_force_relayed,
            None,
        );

        let (command_tx, command_rx) = mpsc::unbounded();
        let mut rpc = MockRpcSession::new();
        rpc.expect_on_connection_loss()
            .return_once(|| stream::pending().boxed_local());
        rpc.expect_on_reconnected()
            .return_once(|| stream::pending().boxed_local());
        rpc.expect_close_with_reason().return_const(());
        rpc.expect_send_command().returning(move |cmd| {
            command_tx.unbounded_send(cmd);
        });

        let component = spawn_component!(
            PeerComponent,
            Rc::new(state),
            peer,
            Rc::new(Ctx {
                connections: Rc::new(Connections::default()),
                rpc: Rc::new(rpc),
            }),
        );

        Ok(Self {
            component,
            command_rx: RefCell::new(Box::pin(command_rx)),
            send_constraints,
            recv_constraints,
        })
    }

    pub async fn get_offer(
        &self,
        tracks: Vec<proto::Track>,
    ) -> Result<String, Traced<MediaConnectionsError>> {
        self.insert_tracks(tracks)?;
        self.component
            .state()
            .set_negotiation_role(NegotiationRole::Offerer);

        while !matches!(
            self.command_rx.borrow_mut().next().await.unwrap(),
            Command::MakeSdpOffer { .. }
        ) {}

        Ok(self.component.state().sdp_offer().unwrap())
    }

    fn insert_tracks(
        &self,
        tracks: Vec<proto::Track>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        for track in tracks {
            match track.direction {
                Direction::Send { mid, receivers } => {
                    self.component.state().insert_sender(
                        track.id,
                        Rc::new(SenderState::new(
                            track.id,
                            mid,
                            track.media_type,
                            receivers,
                            &self.send_constraints,
                        )?),
                    );
                }
                Direction::Recv { mid, sender } => {
                    self.component.state().insert_receiver(
                        track.id,
                        Rc::new(ReceiverState::new(
                            track.id,
                            mid,
                            track.media_type,
                            sender,
                            &self.recv_constraints,
                        )),
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn process_offer(
        &self,
        offer: String,
        tracks: Vec<proto::Track>,
    ) -> Result<String, Traced<MediaConnectionsError>> {
        self.insert_tracks(tracks)?;
        self.component
            .state()
            .set_negotiation_role(NegotiationRole::Answerer(offer));

        while !matches!(
            self.command_rx.borrow_mut().next().await.unwrap(),
            Command::MakeSdpAnswer { .. }
        ) {}

        Ok(self.component.state().sdp_offer().unwrap())
    }

    pub async fn patch_tracks(&self, tracks: Vec<proto::TrackPatchEvent>) {
        for track in tracks {
            if let Some(sender) = self.component.state().get_sender(track.id) {
                sender.update(&track);
            } else if let Some(receiver) =
                self.component.state().get_receiver(track.id)
            {
                receiver.update(&track);
            } else {
                panic!()
            }
        }

        self.component.state().when_all_updated().await;
    }
}

impl Deref for PeerConnectionCompatibility {
    type Target = PeerComponent;

    fn deref(&self) -> &Self::Target {
        &self.component
    }
}
