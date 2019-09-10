//! Service which provide CRUD actions for [`Room`].

use std::{collections::HashMap, marker::PhantomData};

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
            IsEndpointId, IsMemberId, IsRoomId, LocalUri, StatefulLocalUri,
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

type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = RoomService, Item = I, Error = E>>;

/// Errors of [`RoomService`].
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Fail, Display)]
pub enum RoomServiceError {
    /// [`Room`] not found in [`RoomRepository`].
    #[display(fmt = "Room [id = {}] not found.", _0)]
    RoomNotFound(LocalUri<IsRoomId>),

    /// Wrapper for [`Room`]'s [`MailboxError`].
    #[display(fmt = "Room mailbox error: {:?}", _0)]
    RoomMailboxErr(MailboxError),

    /// Try to create [`Room`] with [`RoomId`] which already exists in
    /// [`RoomRepository`].
    #[display(fmt = "Room [id = {}] already exists.", _0)]
    RoomAlreadyExists(LocalUri<IsRoomId>),

    /// Some error happened in [`Room`].
    ///
    /// For more info read [`RoomError`] docs.
    #[display(fmt = "{}", _0)]
    RoomError(RoomError),

    /// Error which can happen while loading static [Control API] specs.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    #[display(fmt = "Failed to load static specs. {:?}", _0)]
    FailedToLoadStaticSpecs(LoadStaticControlSpecsError),

    /// Provided empty [`LocalUri`] list.
    #[display(fmt = "Empty URIs list.")]
    EmptyUrisList,

    /// Provided [`LocalUri`] to some element from [`Room`] but [`Room`] with
    /// ID from this [`LocalUri`] not found in [`RoomRepository`].
    #[display(fmt = "Room not found for element [id = {}]", _0)]
    RoomNotFoundForElement(StatefulLocalUri),

    /// Provided not the same [`RoomId`]s in [`LocalUri`] list.
    ///
    /// Atm this error can happen in `Delete` method because `Delete` should be
    /// called only for one [`Room`].
    #[display(
        fmt = "Provided not the same Room IDs in elements IDs [ids = {:?}].",
        _0
    )]
    NotSameRoomIds(Vec<StatefulLocalUri>, RoomId),
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

    /// Address to [`GracefulShutdown`].
    ///
    /// Use for subscribe newly created [`Room`]s to [`GracefulShutdown`] and
    /// unsubscribe deleted [`Room`]s from [`GracefulShutdown`].
    graceful_shutdown: Addr<GracefulShutdown>,
}

impl RoomService {
    /// Create new [`RoomService`].
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

    /// Closes [`Room`] with provided [`RoomId`].
    ///
    /// This is also deletes this [`Room`] from [`RoomRepository`].
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

/// Implementation of [Control API]'s `Create` method.
///
/// [Control API]: http://tiny.cc/380uaz
#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct CreateRoom {
    pub uri: LocalUri<IsRoomId>,
    pub spec: RoomSpec,
}

impl Handler<CreateRoom> for RoomService {
    type Result = Result<(), RoomServiceError>;

    fn handle(
        &mut self,
        msg: CreateRoom,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let room_id = msg.uri.take_room_id();

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

/// State which indicates that [`DeleteElements`] message was validated and can
/// be send to [`RoomService`].
pub struct Validated;

/// State which indicates that [`DeleteElements`] message is unvalidated and
/// should be validated with `validate()` function of [`DeleteElements`] in
/// [`Unvalidated`] state before sending to [`RoomService`].
pub struct Unvalidated;

#[allow(clippy::use_self)]
impl DeleteElements<Unvalidated> {
    pub fn new() -> DeleteElements<Unvalidated> {
        Self {
            uris: Vec::new(),
            _state: PhantomData,
        }
    }

    pub fn add_uri(&mut self, uri: StatefulLocalUri) {
        self.uris.push(uri)
    }

    // TODO: delete this allow when drain_filter TODO will be resolved.
    #[allow(clippy::unnecessary_filter_map)]
    pub fn validate(
        self,
    ) -> Result<DeleteElements<Validated>, RoomServiceError> {
        if self.uris.is_empty() {
            return Err(RoomServiceError::EmptyUrisList);
        }

        let mut ignored_uris = Vec::new();

        let first_room = self.uris[0].room_id().clone();
        // TODO: rewrite using Vec::drain_filter when it will be in stable
        let uris: Vec<StatefulLocalUri> = self
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

        if !ignored_uris.is_empty() {
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

/// Signal for delete [Control API] elements.
///
/// [Control API]: http://tiny.cc/380uaz
#[derive(Message, Default)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct DeleteElements<T> {
    uris: Vec<StatefulLocalUri>,
    _state: PhantomData<T>,
}

impl Handler<DeleteElements<Validated>> for RoomService {
    type Result = ActFuture<(), RoomServiceError>;

    // TODO: delete this allow when drain_filter TODO will be resolved.
    #[allow(clippy::unnecessary_filter_map)]
    #[allow(clippy::if_not_else)]
    fn handle(
        &mut self,
        msg: DeleteElements<Validated>,
        _: &mut Self::Context,
    ) -> Self::Result {
        if msg.uris.is_empty() {
            return Box::new(actix::fut::err(RoomServiceError::EmptyUrisList));
        }

        let mut deletes_from_room: Vec<StatefulLocalUri> = Vec::new();
        // TODO: use Vec::drain_filter when it will be in stable
        let room_messages_futs: Vec<
            Box<dyn Future<Item = (), Error = MailboxError>>,
        > = msg
            .uris
            .into_iter()
            .filter_map(|l| {
                if let StatefulLocalUri::Room(room_id) = l {
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

/// Implementation of [Control API]'s `Get` method.
///
/// [Control API]: http://tiny.cc/380uaz
#[derive(Message)]
#[rtype(result = "Result<HashMap<StatefulLocalUri, ElementProto>, \
                  RoomServiceError>")]
pub struct Get(pub Vec<StatefulLocalUri>);

impl Handler<Get> for RoomService {
    type Result =
        ActFuture<HashMap<StatefulLocalUri, ElementProto>, RoomServiceError>;

    // TODO: use validation state machine same as Delete method
    fn handle(&mut self, msg: Get, _: &mut Self::Context) -> Self::Result {
        let mut rooms_elements = HashMap::new();
        for uri in msg.0 {
            if self.room_repo.is_contains_room_with_id(uri.room_id()) {
                rooms_elements
                    .entry(uri.room_id().clone())
                    .or_insert_with(Vec::new)
                    .push(uri);
            } else if let StatefulLocalUri::Room(room_uri) = uri {
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
///
/// [`Member`]: crate::signalling::elements::member::Member
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
///
/// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
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
