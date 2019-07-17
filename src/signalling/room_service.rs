//! Service which control [`Room`].

use actix::{
    fut::wrap_future, Actor, ActorFuture, AsyncContext as _, Context, Handler,
    MailboxError, Message,
};
use failure::Fail;
use futures::future::{Either, Future};

use crate::{
    api::control::{
        endpoints::Endpoint as EndpointSpec,
        grpc::protos::control::Element as ElementProto,
        load_static_specs_from_dir,
        local_uri::{IsEndpointId, IsMemberId, IsRoomId, LocalUri},
        MemberId, MemberSpec, RoomId, RoomSpec,
    },
    log::prelude::*,
    signalling::{
        room::{
            Close, CreateEndpoint, CreateMember, DeleteEndpoint, DeleteMember,
            RoomError, SerializeProtobufEndpoint, SerializeProtobufMember,
            SerializeProtobufRoom,
        },
        room_repo::RoomRepository,
        Room,
    },
    AppContext,
};

type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = RoomService, Item = I, Error = E>>;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Fail)]
pub enum RoomServiceError {
    #[fail(display = "Room [id = {}] not found.", _0)]
    RoomNotFound(LocalUri<IsRoomId>),
    #[fail(display = "Mailbox error: {:?}", _0)]
    MailboxError(MailboxError),
    #[fail(display = "Room [id = {}] already exists.", _0)]
    RoomAlreadyExists(LocalUri<IsRoomId>),
    #[fail(display = "{}", _0)]
    RoomError(RoomError),
    #[fail(display = "Failed to load static specs. {:?}", _0)]
    FailedToLoadStaticSpecs(failure::Error),
    #[fail(display = "Unknow error.")]
    Unknow,
}

impl From<RoomError> for RoomServiceError {
    fn from(err: RoomError) -> Self {
        RoomServiceError::RoomError(err)
    }
}

impl From<MailboxError> for RoomServiceError {
    fn from(e: MailboxError) -> Self {
        RoomServiceError::MailboxError(e)
    }
}

/// Service for controlling [`Room`]s.
#[derive(Debug)]
pub struct RoomService {
    /// Repository that stores [`Room`]s addresses.
    room_repo: RoomRepository,

    /// Global app context.
    app: AppContext,
}

impl RoomService {
    pub fn new(room_repo: RoomRepository, app: AppContext) -> Self {
        Self { room_repo, app }
    }
}

impl Actor for RoomService {
    type Context = Context<Self>;
}

/// Returns [`LocalUri`] pointing to [`Room`].
///
/// __Note__ this function don't check presence of [`Room`] in this
/// [`RoomService`].
fn get_local_uri_to_room(room_id: RoomId) -> LocalUri<IsRoomId> {
    LocalUri::<IsRoomId>::new(room_id)
}

/// Signal for load all static specs and start [`Room`]s.
#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct StartStaticRooms;

impl Handler<StartStaticRooms> for RoomService {
    type Result = Result<(), RoomServiceError>;

    fn handle(
        &mut self,
        _: StartStaticRooms,
        _: &mut Self::Context,
    ) -> Self::Result {
        if let Some(static_specs_path) =
            self.app.config.server.static_specs_path.clone()
        {
            let room_specs = match load_static_specs_from_dir(static_specs_path)
            {
                Ok(r) => r,
                Err(e) => {
                    return Err(RoomServiceError::FailedToLoadStaticSpecs(e))
                }
            };

            for spec in room_specs {
                if self.room_repo.is_contains_room_with_id(spec.id()) {
                    return Err(RoomServiceError::RoomAlreadyExists(
                        get_local_uri_to_room(spec.id),
                    ));
                }

                let room_id = spec.id().clone();

                let room = Room::new(&spec, self.app.clone())?.start();

                self.room_repo.add(room_id, room);
            }
        }
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct StartRoom(pub RoomId, pub RoomSpec);

impl Handler<StartRoom> for RoomService {
    type Result = Result<(), RoomServiceError>;

    fn handle(
        &mut self,
        msg: StartRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let room_id = msg.0;

        if self.room_repo.get(&room_id).is_some() {
            return Err(RoomServiceError::RoomAlreadyExists(
                get_local_uri_to_room(room_id),
            ));
        }

        let room = msg.1;

        let room = Room::new(&room, self.app.clone())?;
        let room_addr = room.start();

        debug!("New Room [id = {}] started.", room_id);
        self.room_repo.add(room_id, room_addr);

        Ok(())
    }
}

/// Signal for delete [`Room`].
#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct DeleteRoom(pub RoomId);

impl Handler<DeleteRoom> for RoomService {
    type Result = Result<(), RoomServiceError>;

    fn handle(
        &mut self,
        msg: DeleteRoom,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.room_repo.get(&msg.0) {
            let rooms = self.room_repo.clone();
            ctx.spawn(wrap_future(
                room.send(Close)
                    .map(move |_| {
                        rooms.remove(&msg.0);
                        debug!("Room [id = {}] removed.", msg.0);
                    })
                    .map_err(|e| warn!("Close room mailbox error {:?}.", e)),
            ));
        }

        Ok(())
    }
}

/// Signal for delete [`Member`] from [`Room`].
#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct DeleteMemberFromRoom {
    pub member_id: MemberId,
    pub room_id: RoomId,
}

impl Handler<DeleteMemberFromRoom> for RoomService {
    type Result = Result<(), RoomServiceError>;

    fn handle(
        &mut self,
        msg: DeleteMemberFromRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.room_repo.get(&msg.room_id) {
            room.do_send(DeleteMember(msg.member_id));
        } else {
            return Err(RoomServiceError::RoomNotFound(get_local_uri_to_room(
                msg.room_id,
            )));
        }

        Ok(())
    }
}

/// Signal for delete [`Endpoint`] from [`Member`].
#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct DeleteEndpointFromMember {
    pub room_id: RoomId,
    pub member_id: MemberId,
    pub endpoint_id: String,
}

impl Handler<DeleteEndpointFromMember> for RoomService {
    type Result = Result<(), RoomServiceError>;

    fn handle(
        &mut self,
        msg: DeleteEndpointFromMember,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        if let Some(room) = self.room_repo.get(&msg.room_id) {
            room.do_send(DeleteEndpoint {
                endpoint_id: msg.endpoint_id,
                member_id: msg.member_id,
            });
        }

        Ok(())
    }
}

/// Type alias for result of Get request.
type GetResults = Vec<Result<(String, ElementProto), RoomError>>;

/// Signal for get serialized to protobuf object [`Room`].
#[derive(Message)]
#[rtype(result = "Result<GetResults, RoomServiceError>")]
pub struct GetRoom(pub Vec<RoomId>);

impl Handler<GetRoom> for RoomService {
    type Result = ActFuture<GetResults, RoomServiceError>;

    fn handle(
        &mut self,
        msg: GetRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut futs = Vec::new();

        for room_id in msg.0 {
            if let Some(room) = self.room_repo.get(&room_id) {
                futs.push(
                    room.send(SerializeProtobufRoom)
                        .map_err(RoomServiceError::from)
                        .map(move |result| {
                            result.map(|r| {
                                let local_uri =
                                    LocalUri::<IsRoomId>::new(room_id);
                                (local_uri.to_string(), r)
                            })
                        }),
                )
            } else {
                return Box::new(wrap_future(futures::future::err(
                    RoomServiceError::RoomNotFound(get_local_uri_to_room(
                        room_id,
                    )),
                )));
            }
        }

        Box::new(wrap_future(futures::future::join_all(futs)))
    }
}

/// Signal for get serialized to protobuf object [`Member`].
#[derive(Message)]
#[rtype(result = "Result<GetResults, RoomServiceError>")]
pub struct GetMember(pub Vec<(RoomId, MemberId)>);

impl Handler<GetMember> for RoomService {
    type Result = ActFuture<GetResults, RoomServiceError>;

    fn handle(
        &mut self,
        msg: GetMember,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut futs = Vec::new();

        for (room_id, member_id) in msg.0 {
            if let Some(room) = self.room_repo.get(&room_id) {
                futs.push(
                    room.send(SerializeProtobufMember(member_id.clone()))
                        .map_err(RoomServiceError::from)
                        .map(|result| {
                            result.map(|r| {
                                let local_uri = LocalUri::<IsMemberId>::new(
                                    room_id, member_id,
                                );

                                (local_uri.to_string(), r)
                            })
                        }),
                )
            } else {
                return Box::new(wrap_future(futures::future::err(
                    RoomServiceError::RoomNotFound(get_local_uri_to_room(
                        room_id,
                    )),
                )));
            }
        }

        Box::new(wrap_future(futures::future::join_all(futs)))
    }
}

/// Signal for get serialized to protobuf object `Endpoint`.
#[derive(Message)]
#[rtype(result = "Result<GetResults, RoomServiceError>")]
pub struct GetEndpoint(pub Vec<(RoomId, MemberId, String)>);

impl Handler<GetEndpoint> for RoomService {
    type Result = ActFuture<GetResults, RoomServiceError>;

    fn handle(
        &mut self,
        msg: GetEndpoint,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut futs = Vec::new();

        for (room_id, member_id, endpoint_id) in msg.0 {
            if let Some(room) = self.room_repo.get(&room_id) {
                futs.push(
                    room.send(SerializeProtobufEndpoint(
                        member_id.clone(),
                        endpoint_id.clone(),
                    ))
                    .map_err(RoomServiceError::from)
                    .map(|result| {
                        result.map(|r| {
                            let local_uri = LocalUri::<IsEndpointId>::new(
                                room_id,
                                member_id,
                                endpoint_id,
                            );
                            (local_uri.to_string(), r)
                        })
                    }),
                );
            } else {
                return Box::new(wrap_future(futures::future::err(
                    RoomServiceError::RoomNotFound(get_local_uri_to_room(
                        room_id,
                    )),
                )));
            }
        }

        Box::new(wrap_future(futures::future::join_all(futs)))
    }
}

/// Signal for create new [`Member`] in [`Room`]
#[derive(Message)]
#[rtype(result = "Result<Result<(), RoomError>, RoomServiceError>")]
pub struct CreateMemberInRoom {
    pub room_id: RoomId,
    pub member_id: MemberId,
    pub spec: MemberSpec,
}

impl Handler<CreateMemberInRoom> for RoomService {
    type Result = ActFuture<Result<(), RoomError>, RoomServiceError>;

    fn handle(
        &mut self,
        msg: CreateMemberInRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let fut = if let Some(room) = self.room_repo.get(&msg.room_id) {
            Either::A(
                room.send(CreateMember(msg.member_id, msg.spec))
                    .map_err(RoomServiceError::from),
            )
        } else {
            Either::B(futures::future::err(RoomServiceError::RoomNotFound(
                get_local_uri_to_room(msg.room_id),
            )))
        };

        Box::new(wrap_future(fut))
    }
}

/// Signal for create new [`Endpoint`] in [`Room`]
#[derive(Message)]
#[rtype(result = "Result<Result<(), RoomError>, RoomServiceError>")]
pub struct CreateEndpointInRoom {
    pub room_id: RoomId,
    pub member_id: MemberId,
    pub endpoint_id: String,
    pub spec: EndpointSpec,
}

impl Handler<CreateEndpointInRoom> for RoomService {
    type Result = ActFuture<Result<(), RoomError>, RoomServiceError>;

    fn handle(
        &mut self,
        msg: CreateEndpointInRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let fut = if let Some(room) = self.room_repo.get(&msg.room_id) {
            Either::A(
                room.send(CreateEndpoint {
                    member_id: msg.member_id,
                    endpoint_id: msg.endpoint_id,
                    spec: msg.spec,
                })
                .map_err(RoomServiceError::from),
            )
        } else {
            Either::B(futures::future::err(RoomServiceError::RoomNotFound(
                get_local_uri_to_room(msg.room_id),
            )))
        };

        Box::new(wrap_future(fut))
    }
}
