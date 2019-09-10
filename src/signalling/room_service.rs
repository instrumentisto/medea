//! Service which control [`Room`].

use std::collections::HashMap;

use actix::{
    fut::wrap_future, Actor, ActorFuture, Addr, Context, Handler, MailboxError,
    Message,
};
use derive_more::Display;
use failure::Fail;
use futures::future::{self, Either, Future};
use medea_grpc_proto::control::Element as ElementProto;

use crate::{
    api::control::{
        endpoints::Endpoint as EndpointSpec,
        load_static_specs_from_dir,
        local_uri::{
            IsEndpointId, IsMemberId, IsRoomId, LocalUri, LocalUriType,
        },
        LoadStaticControlSpecsError, MemberSpec, RoomId, RoomSpec,
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
use failure::_core::marker::PhantomData;

type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = RoomService, Item = I, Error = E>>;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Fail, Display)]
pub enum RoomServiceError {
    #[display(fmt = "Room [id = {}] not found.", _0)]
    RoomNotFound(LocalUri<IsRoomId>),
    #[display(fmt = "Room mailbox error: {:?}", _0)]
    RoomMailboxErr(MailboxError),
    #[display(fmt = "Room [id = {}] already exists.", _0)]
    RoomAlreadyExists(LocalUri<IsRoomId>),
    #[display(fmt = "{}", _0)]
    RoomError(RoomError),
    #[display(fmt = "Failed to load static specs. {:?}", _0)]
    FailedToLoadStaticSpecs(LoadStaticControlSpecsError),
    #[display(fmt = "Empty URIs list.")]
    EmptyUrisList,
    #[display(fmt = "Room not found for element [id = {}]", _0)]
    RoomNotFoundForElement(LocalUriType),
    #[display(
        fmt = "Provided not the same Room IDs in elements IDs [ids = {:?}].",
        _0
    )]
    NotSameRoomIds(Vec<LocalUriType>, RoomId),
    #[display(fmt = "Provided Room IDs with Room elements IDs.")]
    DeleteRoomAndFromRoom,
}

impl From<RoomError> for RoomServiceError {
    fn from(err: RoomError) -> Self {
        RoomServiceError::RoomError(err)
    }
}

impl From<LoadStaticControlSpecsError> for RoomServiceError {
    fn from(err: LoadStaticControlSpecsError) -> Self {
        Self::FailedToLoadStaticSpecs(err)
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
        let room_specs = load_static_specs_from_dir(
            self.app.config.control.static_specs_dir.clone(),
        )?;

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
        Ok(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct StartRoom {
    pub id: LocalUri<IsRoomId>,
    pub spec: RoomSpec,
}

impl Handler<StartRoom> for RoomService {
    type Result = Result<(), RoomServiceError>;

    fn handle(
        &mut self,
        msg: StartRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let room_id = msg.id.take_room_id();

        if self.room_repo.get(&room_id).is_some() {
            return Err(RoomServiceError::RoomAlreadyExists(
                get_local_uri_to_room(room_id),
            ));
        }

        let room = Room::new(&msg.spec, self.app.clone())?;
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

pub struct Validated;
pub struct Unvalidated;

impl DeleteElements<Unvalidated> {
    pub fn new() -> DeleteElements<Unvalidated> {
        Self {
            uris: Vec::new(),
            _state: PhantomData,
        }
    }

    pub fn add_uri(&mut self, uri: LocalUriType) {
        self.uris.push(uri)
    }

    pub fn validate(
        self,
    ) -> Result<DeleteElements<Validated>, RoomServiceError> {
        if self.uris.is_empty() {
            return Err(RoomServiceError::EmptyUrisList);
        }

        let mut ignored_uris = Vec::new();

        let first_room = self.uris[0].room_id().clone();
        let uris: Vec<LocalUriType> = self
            .uris
            .into_iter()
            .filter_map(|uri| {
                if uri.room_id() == &first_room {
                    Some(uri)
                } else {
                    ignored_uris.push(uri);
                    None
                }
            })
            .collect();

        if ignored_uris.len() > 0 {
            return Err(RoomServiceError::NotSameRoomIds(
                ignored_uris,
                first_room,
            ));
        }

        Ok(DeleteElements {
            uris,
            _state: PhantomData,
        })
    }
}

/// Signal for delete [`Room`].
#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct DeleteElements<T> {
    uris: Vec<LocalUriType>,
    _state: PhantomData<T>,
}

impl Handler<DeleteElements<Validated>> for RoomService {
    type Result = ActFuture<(), RoomServiceError>;

    // TODO: delete this allow when drain_filter TODO will be resolved.
    #[allow(clippy::unnecessary_filter_map)]
    fn handle(
        &mut self,
        msg: DeleteElements<Validated>,
        _: &mut Self::Context,
    ) -> Self::Result {
        if msg.uris.is_empty() {
            return Box::new(actix::fut::err(RoomServiceError::EmptyUrisList));
        }

        let mut deletes_from_room: Vec<LocalUriType> = Vec::new();
        let room_messages_futs: Vec<
            Box<dyn Future<Item = (), Error = MailboxError>>,
        > = msg
            .uris
            .into_iter()
            .filter_map(|l| {
                if let LocalUriType::Room(room_id) = l {
                    Some(self.close_room(room_id.take_room_id()))
                } else {
                    deletes_from_room.push(l);
                    None
                }
            })
            .collect();

        if !room_messages_futs.is_empty() {
            Box::new(wrap_future(
                futures::future::join_all(room_messages_futs)
                    .map(|_| ())
                    .map_err(RoomServiceError::RoomMailboxErr),
            ))
        } else if !deletes_from_room.is_empty() {
            let room_id = deletes_from_room[0].room_id().clone();

            if let Some(room) = self.room_repo.get(&room_id) {
                Box::new(wrap_future(
                    room.send(Delete(deletes_from_room))
                        .map_err(RoomServiceError::RoomMailboxErr),
                ))
            } else {
                Box::new(actix::fut::err(RoomServiceError::RoomNotFound(
                    get_local_uri_to_room(room_id),
                )))
            }
        } else {
            Box::new(actix::fut::err(RoomServiceError::EmptyUrisList))
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
                    .or_insert_with(Vec::new)
                    .push(uri);
            } else if let LocalUriType::Room(room_uri) = uri {
                return Box::new(actix::fut::err(
                    RoomServiceError::RoomNotFound(room_uri),
                ));
            } else {
                return Box::new(actix::fut::err(
                    RoomServiceError::RoomNotFoundForElement(uri),
                ));
            }
        }

        let mut futs = Vec::new();
        for (room_id, elements) in rooms_elements {
            if let Some(room) = self.room_repo.get(&room_id) {
                futs.push(room.send(SerializeProto(elements)));
            } else {
                return Box::new(actix::fut::err(
                    // TODO: better return RoomNotFoundForElement err
                    RoomServiceError::RoomNotFound(get_local_uri_to_room(
                        room_id,
                    )),
                ));
            }
        }

        Box::new(wrap_future(
            futures::future::join_all(futs)
                .map_err(RoomServiceError::RoomMailboxErr)
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
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct CreateMemberInRoom {
    pub uri: LocalUri<IsMemberId>,
    pub spec: MemberSpec,
}

impl Handler<CreateMemberInRoom> for RoomService {
    type Result = ActFuture<(), RoomServiceError>;

    fn handle(
        &mut self,
        msg: CreateMemberInRoom,
        _: &mut Self::Context,
    ) -> Self::Result {
        let (member_id, room_uri) = msg.uri.take_member_id();
        let room_id = room_uri.take_room_id();

        let fut = if let Some(room) = self.room_repo.get(&room_id) {
            Either::A(
                room.send(CreateMember(member_id, msg.spec))
                    .map_err(RoomServiceError::RoomMailboxErr)
                    .and_then(|r| r.map_err(RoomServiceError::from)),
            )
        } else {
            Either::B(future::err(RoomServiceError::RoomNotFound(
                get_local_uri_to_room(room_id),
            )))
        };

        Box::new(wrap_future(fut))
    }
}

/// Signal for create new [`Endpoint`] in [`Room`]
#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct CreateEndpointInRoom {
    pub uri: LocalUri<IsEndpointId>,
    pub spec: EndpointSpec,
}

impl Handler<CreateEndpointInRoom> for RoomService {
    type Result = ActFuture<(), RoomServiceError>;

    fn handle(
        &mut self,
        msg: CreateEndpointInRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let (endpoint_id, member_uri) = msg.uri.take_endpoint_id();
        let (member_id, room_uri) = member_uri.take_member_id();
        let room_id = room_uri.take_room_id();

        let fut = if let Some(room) = self.room_repo.get(&room_id) {
            Either::A(
                room.send(CreateEndpoint {
                    member_id,
                    endpoint_id,
                    spec: msg.spec,
                })
                .map_err(RoomServiceError::RoomMailboxErr)
                .and_then(|r| r.map_err(RoomServiceError::from)),
            )
        } else {
            Either::B(future::err(RoomServiceError::RoomNotFound(
                get_local_uri_to_room(room_id),
            )))
        };

        Box::new(wrap_future(fut))
    }
}
