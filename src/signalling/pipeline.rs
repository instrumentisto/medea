use super::{
    endpoints_manager::EndpointsManager, members_manager::MembersManager,
    peers::PeerRepository,
};
use crate::{
    api::{
        client::rpc_connection::RpcConnection,
        control::{MemberId, RoomSpec},
    },
    media::IceUser,
    signalling::room::Room,
    turn::service::TurnAuthService,
};
use actix::Context;
use futures::{
    future::{join_all, Either, IntoFuture},
    Future,
};
use hashbrown::{hash_map::IntoIter as _, HashMap};
use std::{cell::RefCell, convert::TryFrom, rc::Rc, time::Duration};
use medea_client_api_proto::Event;
use crate::signalling::room::RoomError;
use crate::signalling::room::ActFuture;
use crate::signalling::members_manager::MemberServiceErr;
use crate::signalling::control::member::Member;

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
        self.members.get_participant_by_id(id).unwrap().is_connected()
    }

    pub fn send_event_to_participant(&mut self, member_id: MemberId, event: Event) -> impl Future<Item = (), Error = RoomError> {
        self.members.send_event_to_participant(member_id, event)
    }

    pub fn get_member_by_id(&self, id: &MemberId) -> &Member {
        self.members.get_participant_by_id(id)
    }

    pub fn connection_established(&mut self, ctx: &mut Context<Room>, id: MemberId, connection: Box<dyn RpcConnection>) -> ActFuture<&Member, MemberServiceErr> {
        self.members.connection_established(ctx, id, connection)
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
