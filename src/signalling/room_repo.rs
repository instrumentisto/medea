//! Repository that stores [`Room`]s addresses.

use std::sync::{Arc, Mutex};

use actix::{
    fut::wrap_future, Actor, ActorFuture, Addr, Context, Handler, MailboxError,
    Message,
};
use failure::Fail;
use futures::future::{Either, Future};
use hashbrown::HashMap;

use crate::{
    api::{
        control::{
            grpc::protos::control::Element as ElementProto,
            local_uri::LocalUri, room::RoomSpec, Endpoint as EndpointSpec,
            MemberId, MemberSpec, RoomId,
        },
        error_codes::ErrorCode,
    },
    signalling::{
        room::{
            CloseRoom, CreateEndpoint, CreateMember, DeleteEndpoint,
            DeleteMember, RoomError, SerializeProtobufEndpoint,
            SerializeProtobufMember, SerializeProtobufRoom,
        },
        Room,
    },
    App,
};

type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = RoomsRepository, Item = I, Error = E>>;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Fail)]
pub enum RoomRepoError {
    #[fail(display = "Room [id = {}] not found.", _0)]
    RoomNotFound(LocalUri),
    #[fail(display = "Mailbox error: {:?}", _0)]
    MailboxError(MailboxError),
    #[fail(display = "Room [id = {}] already exists.", _0)]
    RoomAlreadyExists(LocalUri),
    #[fail(display = "{}", _0)]
    RoomError(RoomError),
    #[fail(display = "Unknow error.")]
    Unknow,
}

impl From<RoomError> for RoomRepoError {
    fn from(err: RoomError) -> Self {
        RoomRepoError::RoomError(err)
    }
}

impl Into<ErrorCode> for RoomRepoError {
    fn into(self) -> ErrorCode {
        match self {
            RoomRepoError::RoomNotFound(id) => ErrorCode::RoomNotFound(id),
            RoomRepoError::RoomAlreadyExists(id) => {
                ErrorCode::RoomAlreadyExists(id)
            }
            RoomRepoError::RoomError(e) => e.into(),
            _ => ErrorCode::UnknownError(self.to_string()),
        }
    }
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

/// Returns [`LocalUri`] pointing to [`Room`].
///
/// __Note__ this function don't check presence of [`Room`] in this
/// [`RoomsRepository`].
fn get_local_uri_to_room(room_id: RoomId) -> LocalUri {
    LocalUri::new(Some(room_id), None, None)
}

#[derive(Message)]
#[rtype(result = "Result<(), RoomRepoError>")]
pub struct StartRoom(pub RoomId, pub RoomSpec);

impl Handler<StartRoom> for RoomsRepository {
    type Result = Result<(), RoomRepoError>;

    fn handle(
        &mut self,
        msg: StartRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let room_id = msg.0;

        if self.rooms.lock().unwrap().get(&room_id).is_some() {
            return Err(RoomRepoError::RoomAlreadyExists(
                get_local_uri_to_room(room_id),
            ));
        }

        let room = msg.1;

        let turn = Arc::clone(&self.app.turn_service);

        let room =
            Room::new(&room, self.app.config.rpc.reconnect_timeout, turn)?;
        let room_addr = room.start();

        self.rooms.lock().unwrap().insert(room_id, room_addr);
        Ok(())
    }
}

/// Signal for delete [`Room`].
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
        let mut room_repo = self.rooms.lock().unwrap();
        if let Some(room) = room_repo.get(&msg.0) {
            room.do_send(CloseRoom {});
            room_repo.remove(&msg.0);
        }

        Ok(())
    }
}

/// Signal for delete [`Member`] from [`Room`].
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
            return Err(RoomRepoError::RoomNotFound(get_local_uri_to_room(
                msg.room_id,
            )));
        }

        Ok(())
    }
}

/// Signal for delete [`Endpoint`] from [`Member`].
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
        }

        Ok(())
    }
}

/// Signal for get serialized to protobuf object [`Room`].
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
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut futs = Vec::new();

        for room_id in msg.0 {
            if let Some(room) = self.rooms.lock().unwrap().get(&room_id) {
                futs.push(
                    room.send(SerializeProtobufRoom)
                        .map_err(RoomRepoError::from)
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
                    RoomRepoError::RoomNotFound(get_local_uri_to_room(room_id)),
                )));
            }
        }

        Box::new(wrap_future(futures::future::join_all(futs)))
    }
}

/// Signal for get serialized to protobuf object [`Member`].
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
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut futs = Vec::new();

        for (room_id, member_id) in msg.0 {
            if let Some(room) = self.rooms.lock().unwrap().get(&room_id) {
                futs.push(
                    room.send(SerializeProtobufMember(member_id.clone()))
                        .map_err(RoomRepoError::from)
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
                    RoomRepoError::RoomNotFound(get_local_uri_to_room(room_id)),
                )));
            }
        }

        Box::new(wrap_future(futures::future::join_all(futs)))
    }
}

/// Signal for get serialized to protobuf object `Endpoint`.
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
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut futs = Vec::new();

        for (room_id, member_id, endpoint_id) in msg.0 {
            if let Some(room) = self.rooms.lock().unwrap().get(&room_id) {
                futs.push(
                    room.send(SerializeProtobufEndpoint(
                        member_id.clone(),
                        endpoint_id.clone(),
                    ))
                    .map_err(RoomRepoError::from)
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
                    RoomRepoError::RoomNotFound(get_local_uri_to_room(room_id)),
                )));
            }
        }

        Box::new(wrap_future(futures::future::join_all(futs)))
    }
}

/// Signal for create new [`Member`] in [`Room`]
#[derive(Message)]
#[rtype(result = "Result<Result<(), RoomError>, RoomRepoError>")]
pub struct CreateMemberInRoom {
    pub room_id: RoomId,
    pub member_id: MemberId,
    pub spec: MemberSpec,
}

impl Handler<CreateMemberInRoom> for RoomsRepository {
    type Result = ActFuture<Result<(), RoomError>, RoomRepoError>;

    fn handle(
        &mut self,
        msg: CreateMemberInRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let fut =
            if let Some(room) = self.rooms.lock().unwrap().get(&msg.room_id) {
                Either::A(
                    room.send(CreateMember(msg.member_id, msg.spec))
                        .map_err(RoomRepoError::from),
                )
            } else {
                Either::B(futures::future::err(RoomRepoError::RoomNotFound(
                    get_local_uri_to_room(msg.room_id),
                )))
            };

        Box::new(wrap_future(fut))
    }
}

/// Signal for create new [`Endpoint`] in [`Room`]
#[derive(Message)]
#[rtype(result = "Result<Result<(), RoomError>, RoomRepoError>")]
pub struct CreateEndpointInRoom {
    pub room_id: RoomId,
    pub member_id: MemberId,
    pub endpoint_id: String,
    pub spec: EndpointSpec,
}

impl Handler<CreateEndpointInRoom> for RoomsRepository {
    type Result = ActFuture<Result<(), RoomError>, RoomRepoError>;

    fn handle(
        &mut self,
        msg: CreateEndpointInRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let fut =
            if let Some(room) = self.rooms.lock().unwrap().get(&msg.room_id) {
                Either::A(
                    room.send(CreateEndpoint {
                        member_id: msg.member_id,
                        endpoint_id: msg.endpoint_id,
                        spec: msg.spec,
                    })
                    .map_err(RoomRepoError::from),
                )
            } else {
                Either::B(futures::future::err(RoomRepoError::RoomNotFound(
                    get_local_uri_to_room(msg.room_id),
                )))
            };

        Box::new(wrap_future(fut))
    }
}
