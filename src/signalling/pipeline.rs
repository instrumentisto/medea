use crate::turn::service::TurnAuthService;
use super::members_manager::MembersManager;
use super::endpoints_manager::EndpointsManager;
use super::peers::PeerRepository;
use crate::api::control::RoomSpec;
use std::convert::TryFrom;
use std::time::Duration;
use hashbrown::HashMap;
use crate::signalling::room::Room;
use actix::Context;
use std::rc::Rc;
use hashbrown::hash_map::IntoIter as _;
use crate::media::IceUser;
use futures::future::{join_all, IntoFuture};
use futures::Future;
use futures::future::Either;
use crate::api::client::rpc_connection::RpcConnection;
use crate::api::control::MemberId;
use std::cell::RefCell;

#[derive(Debug)]
pub struct Pipeline {
    turn: Box<dyn TurnAuthService>,
    members: MembersManager,
    endpoints: EndpointsManager,
    peers: PeerRepository,
}

impl Pipeline {
    pub fn new(turn: Box<dyn TurnAuthService>, reconnect_timeout: Duration, spec: &RoomSpec) -> Self {
        Self {
            turn,
            members: MembersManager::new(spec, reconnect_timeout).unwrap(),
            endpoints: EndpointsManager::new(spec),
            peers: PeerRepository::from(HashMap::new()),
        }
    }

    fn test(&mut self, ice_users: Vec<Rc<RefCell<IceUser>>>) -> impl Future<Item = (), Error = ()>{
        self.turn.delete(ice_users).map_err(|_| ())
    }

    pub fn drop_connections(&mut self, ctx: &mut Context<Room>) -> impl Future<Item = (), Error = ()> {
        let mut fut = Vec::new();

        fut.push(Either::A(self.members.drop_connections(ctx)));
        let ice_users = self.endpoints.take_ice_users();
        let ice_users: Vec<Rc<RefCell<IceUser>>> = ice_users.into_iter().map(|(_, ice_user)| ice_user).collect();

        fut.push(Either::B(self.test(ice_users)));

        join_all(fut).map(|_| ())
    }

    pub fn insert_connection(&mut self, member_id: &MemberId, connection: Box<dyn RpcConnection>) {
        self.members.insert_connection(member_id, connection);
    }
}
