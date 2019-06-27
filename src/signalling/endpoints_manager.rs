use hashbrown::HashMap;
use std::rc::Rc;
use super::control::publish_endpoint::WebRtcPublishEndpoint;
use super::control::play_endpoint::WebRtcPlayEndpoint;
use crate::api::control::RoomSpec;
use crate::api::control::MemberId;
use super::control::play_endpoint::Id as PlayEndpointId;
use super::control::publish_endpoint::Id as PublishEndpointId;
use crate::media::IceUser;
use medea_client_api_proto::IceServer;
use crate::signalling::room::Room;
use futures::Future;
use actix::Context;

#[derive(Debug)]
pub struct EndpointsManager {
    ice_users: HashMap<MemberId, Rc<IceUser>>,
    publishers: HashMap<PublishEndpointId, WebRtcPublishEndpoint>,
    receivers: HashMap<PlayEndpointId, WebRtcPlayEndpoint>,
}

impl EndpointsManager {
    pub fn new(spec: &RoomSpec) -> Self {
        // TODO
        Self {
            ice_users: HashMap::new(),
            publishers: HashMap::new(),
            receivers: HashMap::new(),
        }
    }

    pub fn take_ice_users(&mut self) -> HashMap<MemberId, Rc<IceUser>> {
        let mut ice_users = HashMap::new();
        std::mem::swap(&mut self.ice_users, &mut ice_users);

        ice_users
    }

    pub fn take_ice_user_by_member_id(&mut self, member_id: &MemberId) -> Option<IceUser> {
        self.ice_users.remove(member_id)
    }

    pub fn replace_ice_user(&mut self, member_id: MemberId, mut new_ice_user: Rc<IceUser>) -> Option<Rc<IceUser>> {
        self.ice_users.insert(member_id.clone(), new_ice_user)
    }

    pub fn peers_removed(&mut self, peer_ids: &[PeerId]) {
        self.publishers
            .iter()
            .for_each(|(_, p)| p.remove_peer_ids(peer_ids));

        self.receivers
            .iter()
            .filter_map(|(_, p)| p.peer_id().map(|id| (id, p)))
            .filter(|(id, _)| peer_ids.contains(&id))
            .for_each(|(_, p)| p.reset());
    }

    pub fn get_servers_list_by_member_id(&self, member_id: &MemberId) -> Option<Vec<IceServer>> {
        self.ice_users.get(member_id).as_ref().map(IceUser::servers_list)
    }

    pub fn insert_receiver(&mut self, id: PlayEndpointId, receiver: WebRtcPlayEndpoint) {
        self.receivers.insert(id, receiver);
    }

    pub fn insert_publisher(&mut self, id: PublishEndpointId, publisher: WebRtcPublishEndpoint) {
        self.publishers.insert(id, publisher);
    }

    pub fn get_publisher_by_id(&self, id: &PublishEndpointId) -> &WebRtcPublishEndpoint {
        self.publishers.get(id)
    }

    pub fn get_receiver_by_id(&self, id: &PlayEndpointId) -> &WebRtcPlayEndpoint {
        self.receivers.get(id)
    }
}
