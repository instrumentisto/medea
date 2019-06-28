use super::{
    endpoints_manager::EndpointsManager, members_manager::MembersManager,
    peers::PeerRepository,
};
use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, ClosedReason, RpcConnection,
        },
        control::{MemberId, RoomId, RoomSpec},
    },
    log::prelude::*,
    media::IceUser,
    signalling::{
        control::{
            member::Member,
            play_endpoint::{Id as PlayEndpointId, WebRtcPlayEndpoint},
            publish_endpoint::{
                Id as PublishEndpointId, WebRtcPublishEndpoint,
            },
        },
        members_manager::MemberServiceErr,
        room::{ActFuture, Room, RoomError},
    },
    turn::{service::TurnAuthService, TurnServiceErr, UnreachablePolicy},
};
use actix::{fut::wrap_future, AsyncContext, Context};
use futures::{
    future::{self, join_all, Either},
    Future,
};
use hashbrown::HashMap;
use medea_client_api_proto::{Event, IceServer};
use std::{cell::RefCell, rc::Rc, time::Duration};
use crate::media::PeerId;

#[derive(Debug)]
pub struct Pipeline {
    turn: Box<dyn TurnAuthService>,
    members: MembersManager,
    endpoints: EndpointsManager,
    peers: PeerRepository,
}

impl Pipeline {
    pub fn new(
        turn: Box<dyn TurnAuthService>,
        reconnect_timeout: Duration,
        spec: &RoomSpec,
    ) -> Self {
        Self {
            turn,
            members: MembersManager::new(spec, reconnect_timeout).unwrap(),
            endpoints: EndpointsManager::new(spec),
            peers: PeerRepository::from(HashMap::new()),
        }
    }

    pub fn is_member_has_connection(&self, id: &MemberId) -> bool {
        self.members
            .get_participant_by_id(id)
            .unwrap()
            .borrow()
            .is_connected()
    }

    pub fn send_event_to_participant(
        &mut self,
        member_id: MemberId,
        event: Event,
    ) -> impl Future<Item = (), Error = RoomError> {
        self.members.send_event_to_participant(member_id, event)
    }

    pub fn get_member_by_id(
        &self,
        id: &MemberId,
    ) -> Option<Rc<RefCell<Member>>> {
        self.members.get_participant_by_id(id)
    }

    pub fn get_member_by_id_and_credentials(
        &self,
        id: &MemberId,
        credentials: &str,
    ) -> Result<Rc<RefCell<Member>>, AuthorizationError> {
        self.members
            .get_participant_by_id_and_credentials(id, credentials)
    }

    pub fn connection_closed(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
        close_reason: ClosedReason,
    ) {
        ctx.spawn(wrap_future(
            self.delete_ice_user(&member_id)
                .map_err(|err| error!("Error deleting IceUser {:?}", err)),
        ));
        self.members
            .connection_closed(ctx, &member_id, &close_reason);
    }

    pub fn delete_ice_user(
        &mut self,
        member_id: &MemberId,
    ) -> Box<dyn Future<Item = (), Error = TurnServiceErr>> {
        let ice_user = self.endpoints.take_ice_user_by_member_id(member_id);
        match self.get_member_by_id(member_id) {
            Some(participant) => match ice_user {
                Some(ice_user) => self.turn.delete(vec![ice_user]),
                None => Box::new(future::ok(())),
            },
            None => Box::new(future::ok(())),
        }
    }

    pub fn get_publishers_by_member_id(
        &self,
        id: &MemberId,
    ) -> HashMap<&PublishEndpointId, Rc<RefCell<WebRtcPublishEndpoint>>> {
        self.endpoints.get_publishers_by_member_id(id)
    }

    pub fn endpoints_manager(&mut self) -> &mut EndpointsManager {
        &mut self.endpoints
    }

    pub fn get_receivers_by_member_id(
        &self,
        id: &MemberId,
    ) -> HashMap<&PlayEndpointId, Rc<RefCell<WebRtcPlayEndpoint>>> {
        self.endpoints.get_receivers_by_member_id(id)
    }

    pub fn create_turn(
        &self,
        member_id: MemberId,
        room_id: RoomId,
        policy: UnreachablePolicy,
    ) -> Box<dyn Future<Item = IceUser, Error = TurnServiceErr>> {
        self.turn.create(member_id, room_id, policy)
    }

    pub fn replace_ice_user(
        &mut self,
        member_id: &MemberId,
        ice_user: Rc<RefCell<IceUser>>,
    ) -> Option<Rc<RefCell<IceUser>>> {
        self.endpoints.replace_ice_user(member_id.clone(), ice_user)
    }

    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        id: MemberId,
        connection: Box<dyn RpcConnection>,
    ) -> ActFuture<&Member, MemberServiceErr> {
        // self.members.connection_established(ctx, &self, id, connection)
    }

    pub fn get_ice_servers(&self, id: &MemberId) -> Option<Vec<IceServer>> {
        self.endpoints.get_servers_list_by_member_id(id)
    }

    pub fn peers_removed(&mut self, peers_id: &[PeerId]) {
        self.endpoints.peers_removed(peers_id)
    }

    pub fn get_receiver_by_id(
        &self,
        id: &PlayEndpointId,
    ) -> Option<Rc<RefCell<WebRtcPlayEndpoint>>> {
        self.endpoints.get_receiver_by_id(id)
    }

    pub fn get_publisher_by_id(
        &self,
        id: &PublishEndpointId,
    ) -> Option<Rc<RefCell<WebRtcPublishEndpoint>>> {
        self.endpoints.get_publisher_by_id(id)
    }

    fn test(
        &mut self,
        ice_users: Vec<Rc<RefCell<IceUser>>>,
    ) -> impl Future<Item = (), Error = ()> {
        self.turn.delete(ice_users).map_err(|_| ())
    }

    pub fn drop_connections(
        &mut self,
        ctx: &mut Context<Room>,
    ) -> impl Future<Item = (), Error = ()> {
        let mut fut = Vec::new();

        fut.push(Either::A(self.members.drop_connections(ctx)));
        let ice_users = self.endpoints.take_ice_users();
        let ice_users: Vec<Rc<RefCell<IceUser>>> = ice_users
            .into_iter()
            .map(|(_, ice_user)| ice_user)
            .collect();

        fut.push(Either::B(self.test(ice_users)));

        join_all(fut).map(|_| ())
    }

    pub fn insert_connection(
        &mut self,
        member_id: &MemberId,
        connection: Box<dyn RpcConnection>,
    ) {
        self.members.insert_connection(member_id, connection);
    }
}
