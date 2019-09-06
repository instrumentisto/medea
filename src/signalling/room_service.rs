//! Service which control [`Room`].

use actix::{
    fut::wrap_future, Actor, ActorFuture, Addr, Context, Handler, MailboxError,
    Message,
};
use failure::Fail;
use futures::future::{self, Either, Future};
use medea_grpc_proto::control::Element as ElementProto;

use crate::{
    api::control::{
        endpoints::Endpoint as EndpointSpec,
        load_static_specs_from_dir,
        local_uri::{IsRoomId, LocalUri, LocalUriType},
        MemberId, MemberSpec, RoomId, RoomSpec,
    },
    log::prelude::*,
    shutdown::{self, GracefulShutdown},
    signalling::{
        room::{
            Close, CreateEndpoint, CreateMember, Delete, RoomError,
            SerializeProto,
        },
        room_repo::RoomRepository,
        Room,
    },
    AppContext,
};
use serde::export::PhantomData;
use std::collections::HashMap;

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
    Unknown,
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
pub struct RoomService {
    /// Repository that stores [`Room`]s addresses.
    room_repo: RoomRepository,

    /// Global app context.
    app: AppContext,

    graceful_shutdown: Addr<GracefulShutdown>,
}

impl RoomService {
    pub fn new(
        room_repo: RoomRepository,
        app: AppContext,
        graceful_shutdown: Addr<GracefulShutdown>,
    ) -> Self {
        Self {
            room_repo,
            app,
            graceful_shutdown,
        }
    }

    fn close_room(
        &self,
        id: RoomId,
    ) -> Box<dyn Future<Item = (), Error = MailboxError>> {
        if let Some(room) = self.room_repo.get(&id) {
            shutdown::unsubscribe(
                &self.graceful_shutdown,
                room.clone().recipient(),
                shutdown::Priority(2),
            );

            let room_repo = self.room_repo.clone();

            Box::new(room.send(Close).map(move |_| {
                debug!("Room [id = {}] removed.", id);
                room_repo.remove(&id);
            }))
        } else {
            Box::new(futures::future::ok(()))
        }
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
            self.app.config.server.http.static_specs_path.clone()
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
                shutdown::subscribe(
                    &self.graceful_shutdown,
                    room.clone().recipient(),
                    shutdown::Priority(2),
                );

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

        let room = Room::new(&msg.1, self.app.clone())?;
        let room_addr = room.start();

        shutdown::subscribe(
            &self.graceful_shutdown,
            room_addr.clone().recipient(),
            shutdown::Priority(2),
        );

        debug!("New Room [id = {}] started.", room_id);
        self.room_repo.add(room_id, room_addr);

        Ok(())
    }
}

/// Signal for delete [`Room`].
#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct DeleteElements<T> {
    uris: Vec<LocalUriType>,
    _marker: PhantomData<T>,
}

pub struct NotValidated;
pub struct Valid;

impl DeleteElements<NotValidated> {
    pub fn new() -> DeleteElements<NotValidated> {
        Self {
            uris: vec![],
            _marker: PhantomData,
        }
    }

    pub fn add_uri(&mut self, uri: LocalUriType) {
        self.uris.push(uri);
    }

    pub fn validate(self) -> Result<DeleteElements<Valid>, RoomServiceError> {
        // TODO: correct errors
        if self.uris.is_empty() {
            return Err(RoomServiceError::Unknown);
        }

        let first_room = self.uris[0].room_id();
        let is_same_room =
            self.uris.iter().all(|item| item.room_id() == first_room);

        if !is_same_room {
            return Err(RoomServiceError::Unknown);
        }

        Ok(DeleteElements {
            uris: self.uris,
            _marker: PhantomData,
        })
    }
}

impl Handler<DeleteElements<Valid>> for RoomService {
    type Result = ActFuture<(), RoomServiceError>;

    fn handle(
        &mut self,
        msg: DeleteElements<Valid>,
        _: &mut Context<RoomService>,
    ) -> Self::Result {
        let mut deletes_from_room: Vec<LocalUriType> = Vec::new();
        let room_messages_futs: Vec<
            Box<dyn Future<Item = (), Error = MailboxError>>,
        > = msg
            .uris
            .into_iter()
            .filter_map(|l| {
                if let LocalUriType::Room(room_id) = l {
                    Some(room_id)
                } else {
                    deletes_from_room.push(l);
                    None
                }
            })
            .map(|room_id| self.close_room(room_id.take_room_id()))
            .collect();

        if !room_messages_futs.is_empty() {
            Box::new(wrap_future(
                futures::future::join_all(room_messages_futs)
                    .map(|_| ())
                    .map_err(|e| RoomServiceError::from(e)),
            ))
        } else if !deletes_from_room.is_empty() {
            let room_id = deletes_from_room[0].room_id().clone();
            let mut ignored_ids = Vec::new();
            let deletes_from_room: Vec<LocalUriType> = deletes_from_room
                .into_iter()
                .filter_map(|uri| {
                    if uri.room_id() == &room_id {
                        Some(uri)
                    } else {
                        ignored_ids.push(uri);
                        None
                    }
                })
                .collect();
            if !ignored_ids.is_empty() {
                warn!(
                    "Some ids from Get request was ignored: {:?}",
                    ignored_ids
                );
            }
            if let Some(room) = self.room_repo.get(&room_id) {
                Box::new(wrap_future(
                    room.send(Delete(deletes_from_room))
                        .map_err(|e| RoomServiceError::from(e)),
                ))
            } else {
                Box::new(actix::fut::err(RoomServiceError::RoomNotFound(
                    get_local_uri_to_room(room_id),
                )))
            }
        } else {
            Box::new(actix::fut::ok(()))
        }
    }
}

#[derive(Message)]
#[rtype(
    result = "Result<HashMap<LocalUriType, ElementProto>, RoomServiceError>"
)]
pub struct Get(pub Vec<LocalUriType>);

impl Handler<Get> for RoomService {
    type Result =
        ActFuture<HashMap<LocalUriType, ElementProto>, RoomServiceError>;

    fn handle(&mut self, msg: Get, _: &mut Self::Context) -> Self::Result {
        let mut rooms_elements = HashMap::new();
        for uri in msg.0 {
            if self.room_repo.is_contains_room_with_id(uri.room_id()) {
                rooms_elements
                    .entry(uri.room_id().clone())
                    .or_insert_with(|| Vec::new())
                    .push(uri);
            } else {
                if let LocalUriType::Room(room_uri) = uri {
                    return Box::new(actix::fut::err(
                        RoomServiceError::RoomNotFound(room_uri),
                    ));
                } else {
                    return Box::new(actix::fut::err(
                        RoomServiceError::Unknown,
                    ));
                }
            }
        }

        let mut futs = Vec::new();
        for (room_id, elements) in rooms_elements {
            if let Some(room) = self.room_repo.get(&room_id) {
                futs.push(room.send(SerializeProto(elements)));
            } else {
                return Box::new(actix::fut::err(
                    RoomServiceError::RoomNotFound(get_local_uri_to_room(
                        room_id,
                    )),
                ));
            }
        }

        Box::new(wrap_future(
            futures::future::join_all(futs)
                .map_err(|e| RoomServiceError::from(e))
                .and_then(|results| {
                    let mut all = HashMap::new();
                    for result in results {
                        match result {
                            Ok(res) => all.extend(res),
                            Err(e) => return Err(RoomServiceError::from(e)),
                        }
                    }
                    Ok(all)
                }),
        ))
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
        _: &mut Self::Context,
    ) -> Self::Result {
        let fut = if let Some(room) = self.room_repo.get(&msg.room_id) {
            Either::A(
                room.send(CreateMember(msg.member_id, msg.spec))
                    .map_err(RoomServiceError::from),
            )
        } else {
            Either::B(future::err(RoomServiceError::RoomNotFound(
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
            Either::B(future::err(RoomServiceError::RoomNotFound(
                get_local_uri_to_room(msg.room_id),
            )))
        };

        Box::new(wrap_future(fut))
    }
}
