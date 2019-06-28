use super::control::{
    play_endpoint::{Id as PlayEndpointId, WebRtcPlayEndpoint},
    publish_endpoint::{Id as PublishEndpointId, WebRtcPublishEndpoint},
};
use crate::{
    api::control::{MemberId, RoomSpec},
    media::{IceUser, PeerId},
    signalling::room::Room,
};
use actix::Context;
use futures::Future;
use hashbrown::HashMap;
use medea_client_api_proto::IceServer;
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub struct EndpointsManager {
    ice_users: HashMap<MemberId, Rc<RefCell<IceUser>>>,
    publishers: HashMap<PublishEndpointId, Rc<RefCell<WebRtcPublishEndpoint>>>,
    receivers: HashMap<PlayEndpointId, Rc<RefCell<WebRtcPlayEndpoint>>>,
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


    // TODO: rename
    pub fn get_publish_sinks(&mut self, member_id: &MemberId, partner_id: &MemberId) -> Vec<Rc<RefCell<WebRtcPlayEndpoint>>> {
        self.get_publishers_by_member_id(member_id)
            .into_iter()
            .flat_map(|(_, p)| p.borrow().sinks().into_iter())
            .filter_map(|id| self.get_receiver_by_id(id))
            .collect()
    }

    pub fn take_ice_users(
        &mut self,
    ) -> HashMap<MemberId, Rc<RefCell<IceUser>>> {
        let mut ice_users = HashMap::new();
        std::mem::swap(&mut self.ice_users, &mut ice_users);

        ice_users
    }

    pub fn get_publishers_by_member_id(
        &self,
        id: &MemberId,
    ) -> HashMap<PublishEndpointId, Rc<RefCell<WebRtcPublishEndpoint>>> {
        self.publishers
            .iter()
            .map(|(id, p)| (id.clone(), p.clone()))
            .filter(|(id, p)| p.borrow().owner() == id)
            .collect()
    }

    pub fn get_receivers_by_member_id(
        &self,
        id: &MemberId,
    ) -> HashMap<&PlayEndpointId, Rc<RefCell<WebRtcPlayEndpoint>>> {
        self.receivers
            .iter()
            .map(|(id, p)| (id, Rc::clone(p)))
            .filter(|(_, p)| p.borrow().owner() == id)
            .collect()
    }

    pub fn take_ice_user_by_member_id(
        &mut self,
        member_id: &MemberId,
    ) -> Option<Rc<RefCell<IceUser>>> {
        self.ice_users.remove(member_id)
    }

    pub fn replace_ice_user(
        &mut self,
        member_id: MemberId,
        mut new_ice_user: Rc<RefCell<IceUser>>,
    ) -> Option<Rc<RefCell<IceUser>>> {
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

    pub fn get_servers_list_by_member_id(
        &self,
        member_id: &MemberId,
    ) -> Option<Vec<IceServer>> {
        self.ice_users
            .get(member_id)
            .as_ref()
            .map(|u| u.borrow().servers_list())
    }

    pub fn insert_receiver(
        &mut self,
        id: PlayEndpointId,
        receiver: WebRtcPlayEndpoint,
    ) {
        self.receivers.insert(id, Rc::new(RefCell::new(receiver)));
    }

    pub fn insert_publisher(
        &mut self,
        id: PublishEndpointId,
        publisher: WebRtcPublishEndpoint,
    ) {
        self.publishers.insert(id, Rc::new(RefCell::new(publisher)));
    }

    pub fn get_publisher_by_id(
        &self,
        id: &PublishEndpointId,
    ) -> Option<Rc<RefCell<WebRtcPublishEndpoint>>> {
        self.publishers.get(id).map(Rc::clone)
    }

    pub fn get_receiver_by_id(
        &self,
        id: &PlayEndpointId,
    ) -> Option<Rc<RefCell<WebRtcPlayEndpoint>>> {
        self.receivers.get(id).map(Rc::clone)
    }
}
