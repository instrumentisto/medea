use futures::Future;
use hashbrown::HashMap;

use crate::api::client::{Event, RpcConnection};
use crate::api::control::member::Id as MemberId;
use crate::media::peer::{Id as PeerId, PeerMachine};

#[derive(Debug)]
pub struct Session {
    pub member_id: MemberId,
    pub connection: Box<dyn RpcConnection>,
    pub peers: HashMap<PeerId, PeerMachine>,
}

impl Session {
    pub fn new(
        member_id: MemberId,
        connection: Box<dyn RpcConnection>,
    ) -> Self {
        Session {
            member_id,
            connection,
            peers: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, peer: PeerMachine) {
        self.peers.insert(peer.id(), peer);
    }

    pub fn remove_peer(&mut self, peer_id: PeerId) -> Option<PeerMachine> {
        self.peers.remove(&peer_id)
    }

    pub fn send_event(
        &self,
        event: Event,
    ) -> Box<dyn Future<Item = (), Error = ()>> {
        self.connection.send_event(event)
    }

    pub fn set_connection(
        &mut self,
        connection: Box<dyn RpcConnection>,
    ) -> Box<dyn Future<Item = (), Error = ()>> {
        let fut = self.connection.close();
        self.connection = connection;
        fut
    }
}
