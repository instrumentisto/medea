use hashbrown::HashMap;
use std::rc::Rc;
use super::control::publish_endpoint::WebRtcPublishEndpoint;
use super::control::play_endpoint::WebRtcPlayEndpoint;
use crate::api::control::RoomSpec;
use crate::api::control::MemberId;
use super::control::play_endpoint::Id as PlayEndpointId;
use super::control::publish_endpoint::Id as PublishEndpointId;
use crate::media::IceUser;
use crate::signalling::room::Room;
use futures::Future;
use actix::Context;
use std::iter::Iterator;
use std::iter::IntoIterator;

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

//    pub fn drop_connections(&mut self, ctx: &mut Context<Room>) -> impl Future<Item = (), Error = ()> {
//        let remove_ice_users = Box::new({
//            let mut ice_users = HashMap::new();
//            std::mem::swap(&mut self.ice_users, &mut ice_users);
//            let ice_users: Vec<Rc<IceUser>> = ice_users.into_iter().map(|(_, ice_user)| ice_user).collect();
//
//        });
////        let remove_ice_users = Box::new({
////            let mut room_users = Vec::with_capacity(self.participants.len());
////
////            self.participants.iter().for_each(|(_, data)| {
////                if let Some(ice_user) = data.take_ice_user() {
////                    room_users.push(ice_user);
////                }
////            });
////            self.turn
////                .delete(room_users)
////                .map_err(|err| error!("Error removing IceUsers {:?}", err))
////        });
////        close_fut.push(remove_ice_users);
////
////        join_all(close_fut).map(|_| ())
//    }
}
