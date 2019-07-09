//! Repository that stores [`Room`]s addresses.

use std::{
    collections::HashMap as StdHashMap,
    sync::{Arc, Mutex},
};

use actix::{
    Actor, ActorFuture, Addr, Context, Handler, MailboxError, Message,
};
use failure::Fail;
use hashbrown::HashMap;

use crate::{
    api::control::{
        grpc::protos::control::Element as ElementProto, local_uri::LocalUri,
        room::RoomSpec, MemberId, RoomId,
    },
    signalling::{
        room::{
            CloseRoom, DeleteEndpoint, DeleteMember, RoomError, Serialize,
            SerializeEndpoint, SerializeMember,
        },
        Room,
    },
    App,
};
use actix::fut::wrap_future;
use futures::future::{Either, Future};

type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = RoomsRepository, Item = I, Error = E>>;

#[derive(Debug, Fail)]
pub enum RoomRepoError {
    #[fail(display = "Room with id {} not found.", _0)]
    RoomNotFound(RoomId),
    #[fail(display = "Mailbox error: {:?}", _0)]
    MailboxError(MailboxError),
    #[fail(display = "Unknow error.")]
    Unknow,
}

impl From<MailboxError> for RoomRepoError {
    fn from(e: MailboxError) -> Self {
        RoomRepoError::MailboxError(e)
    }
}

/// Repository that stores [`Room`]s addresses.
#[derive(Clone, Debug)]
pub struct RoomsRepository {
    // TODO: Use crossbeam's concurrent hashmap when its done.
    //       [Tracking](https://github.com/crossbeam-rs/rfcs/issues/32).
    rooms: Arc<Mutex<HashMap<RoomId, Addr<Room>>>>,
    app: Arc<App>,
}

impl RoomsRepository {
    /// Creates new [`Room`]s repository with passed-in [`Room`]s.
    pub fn new(rooms: HashMap<RoomId, Addr<Room>>, app: Arc<App>) -> Self {
        Self {
            rooms: Arc::new(Mutex::new(rooms)),
            app,
        }
    }

    /// Returns [`Room`] by its ID.
    pub fn get(&self, id: &RoomId) -> Option<Addr<Room>> {
        let rooms = self.rooms.lock().unwrap();
        rooms.get(id).cloned()
    }

    pub fn remove(&self, id: &RoomId) {
        self.rooms.lock().unwrap().remove(id);
    }

    pub fn add(&self, id: RoomId, room: Addr<Room>) {
        self.rooms.lock().unwrap().insert(id, room);
    }
}

impl Actor for RoomsRepository {
    type Context = Context<Self>;
}

// TODO: return sids.
#[derive(Message)]
#[rtype(result = "Result<(), RoomError>")]
pub struct StartRoom(pub RoomId, pub RoomSpec);

impl Handler<StartRoom> for RoomsRepository {
    type Result = Result<(), RoomError>;

    fn handle(
        &mut self,
        msg: StartRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let room_id = msg.0;
        let room = msg.1;

        let turn = Arc::clone(&self.app.turn_service);

        let room = Room::new(
            &room,
            self.app.config.rpc.reconnect_timeout.clone(),
            turn,
        )?;
        let room_addr = room.start();

        self.rooms.lock().unwrap().insert(room_id, room_addr);
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), RoomRepoError>")]
pub struct DeleteRoom(pub RoomId);

impl Handler<DeleteRoom> for RoomsRepository {
    type Result = Result<(), RoomRepoError>;

    fn handle(
        &mut self,
        msg: DeleteRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.rooms.lock().unwrap().get(&msg.0) {
            room.do_send(CloseRoom {});
        } else {
            return Err(RoomRepoError::RoomNotFound(msg.0));
        }

        self.remove(&msg.0);

        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), RoomRepoError>")]
pub struct DeleteMemberFromRoom {
    pub member_id: MemberId,
    pub room_id: RoomId,
}

impl Handler<DeleteMemberFromRoom> for RoomsRepository {
    type Result = Result<(), RoomRepoError>;

    fn handle(
        &mut self,
        msg: DeleteMemberFromRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.get(&msg.room_id) {
            room.do_send(DeleteMember(msg.member_id));
        } else {
            return Err(RoomRepoError::RoomNotFound(msg.room_id));
        }

        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), RoomRepoError>")]
pub struct DeleteEndpointFromMember {
    pub room_id: RoomId,
    pub member_id: MemberId,
    pub endpoint_id: String,
}

impl Handler<DeleteEndpointFromMember> for RoomsRepository {
    type Result = Result<(), RoomRepoError>;

    fn handle(
        &mut self,
        msg: DeleteEndpointFromMember,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.get(&msg.room_id) {
            room.do_send(DeleteEndpoint {
                endpoint_id: msg.endpoint_id,
                member_id: msg.member_id,
            });
        } else {
            return Err(RoomRepoError::RoomNotFound(msg.room_id));
        }

        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<Result<(String, ElementProto), RoomError>>, \
                  RoomRepoError>")]
pub struct GetRoom(pub Vec<RoomId>);

impl Handler<GetRoom> for RoomsRepository {
    type Result = ActFuture<
        Vec<Result<(String, ElementProto), RoomError>>,
        RoomRepoError,
    >;

    fn handle(
        &mut self,
        msg: GetRoom,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut futs = Vec::new();

        for room_id in msg.0 {
            if let Some(room) = self.rooms.lock().unwrap().get(&room_id) {
                futs.push(
                    room.send(Serialize)
                        .map_err(|e| RoomRepoError::from(e))
                        .map(move |result| {
                            result.map(|r| {
                                let local_uri = LocalUri {
                                    room_id: Some(room_id),
                                    member_id: None,
                                    endpoint_id: None,
                                };
                                (local_uri.to_string(), r)
                            })
                        }),
                )
            } else {
                return Box::new(wrap_future(futures::future::err(
                    RoomRepoError::RoomNotFound(room_id),
                )));
            }
        }

        Box::new(wrap_future(futures::future::join_all(futs)))
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<Result<(String, ElementProto), RoomError>>, \
                  RoomRepoError>")]
pub struct GetMember(pub Vec<(RoomId, MemberId)>);

impl Handler<GetMember> for RoomsRepository {
    type Result = ActFuture<
        Vec<Result<(String, ElementProto), RoomError>>,
        RoomRepoError,
    >;

    fn handle(
        &mut self,
        msg: GetMember,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut futs = Vec::new();

        for (room_id, member_id) in msg.0 {
            if let Some(room) = self.rooms.lock().unwrap().get(&room_id) {
                futs.push(
                    room.send(SerializeMember(member_id.clone()))
                        .map_err(|e| RoomRepoError::from(e))
                        .map(|result| {
                            result.map(|r| {
                                let local_uri = LocalUri {
                                    room_id: Some(room_id),
                                    member_id: Some(member_id),
                                    endpoint_id: None,
                                };

                                (local_uri.to_string(), r)
                            })
                        }),
                )
            } else {
                return Box::new(wrap_future(futures::future::err(
                    RoomRepoError::RoomNotFound(room_id),
                )));
            }
        }

        Box::new(wrap_future(futures::future::join_all(futs)))
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<Result<(String, ElementProto), RoomError>>, \
                  RoomRepoError>")]
pub struct GetEndpoint(pub Vec<(RoomId, MemberId, String)>);

impl Handler<GetEndpoint> for RoomsRepository {
    type Result = ActFuture<
        Vec<Result<(String, ElementProto), RoomError>>,
        RoomRepoError,
    >;

    fn handle(
        &mut self,
        msg: GetEndpoint,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut futs = Vec::new();

        for (room_id, member_id, endpoint_id) in msg.0 {
            if let Some(room) = self.rooms.lock().unwrap().get(&room_id) {
                futs.push(
                    room.send(SerializeEndpoint(
                        member_id.clone(),
                        endpoint_id.clone(),
                    ))
                    .map_err(|e| RoomRepoError::from(e))
                    .map(|result| {
                        result.map(|r| {
                            let local_uri = LocalUri {
                                room_id: Some(room_id),
                                member_id: Some(member_id),
                                endpoint_id: Some(endpoint_id),
                            };
                            (local_uri.to_string(), r)
                        })
                    }),
                );
            } else {
                return Box::new(wrap_future(futures::future::err(
                    RoomRepoError::RoomNotFound(room_id),
                )));
            }
        }

        Box::new(wrap_future(futures::future::join_all(futs)))
    }
}
