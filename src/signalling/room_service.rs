//! Service which provides CRUD actions for [`Room`].

use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use actix::{
    Actor, Addr, Context, Handler, MailboxError, Message, ResponseFuture,
};
use derive_more::Display;
use failure::Fail;
use futures::future::{
    self, FutureExt as _, LocalBoxFuture, TryFutureExt as _,
};
use medea_client_api_proto::{MemberId, RoomId};
use medea_control_api_proto::grpc::api as proto;
use redis::RedisError;

use crate::{
    api::control::{
        endpoints::EndpointSpec,
        load_static_specs_from_dir,
        member::Sid,
        refs::{Fid, StatefulFid, ToMember, ToRoom},
        EndpointId, LoadStaticControlSpecsError, MemberSpec, RoomSpec,
        TryFromElementError,
    },
    conf::server::PublicUrl,
    log::prelude::*,
    shutdown::{self, GracefulShutdown},
    signalling::{
        peers::{build_peers_traffic_watcher, PeerTrafficWatcher},
        room::{
            Apply, Close, CreateEndpoint, CreateMember, Delete, RoomError,
            SerializeProto,
        },
        room_repo::RoomRepository,
        Room,
    },
    AppContext,
};

/// Errors of [`RoomService`].
#[derive(Debug, Fail, Display)]
pub enum RoomServiceError {
    /// [`Room`] not found in [`RoomRepository`].
    #[display(fmt = "Room [id = {}] not found.", _0)]
    RoomNotFound(Fid<ToRoom>),

    /// Wrapper for [`Room`]'s [`MailboxError`].
    #[display(fmt = "Room mailbox error: {:?}", _0)]
    RoomMailboxErr(MailboxError),

    /// Attempt to create [`Room`] with [`RoomId`] which already exists in
    /// [`RoomRepository`].
    #[display(fmt = "Room [id = {}] already exists.", _0)]
    RoomAlreadyExists(Fid<ToRoom>),

    /// Some error happened in [`Room`].
    ///
    /// For more info read [`RoomError`] docs.
    RoomError(RoomError),

    /// Error which can happen while converting protobuf objects into interior
    /// [medea] [Control API] objects.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    /// [medea]: https://github.com/instrumentisto/medea
    TryFromElement(TryFromElementError),

    /// Error which can happen while loading static [Control API] specs.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    #[display(fmt = "Failed to load static specs. {:?}", _0)]
    FailedToLoadStaticSpecs(LoadStaticControlSpecsError),

    /// Provided empty [`Fid`] list.
    #[display(fmt = "Empty URIs list.")]
    EmptyUrisList,

    /// Provided not the same [`RoomId`]s in [`Fid`] list.
    ///
    /// Atm this error can happen in `Delete` method because `Delete` should be
    /// called only for one [`Room`].
    #[display(
        fmt = "Provided not the same Room IDs in elements IDs [ids = {:?}].",
        _1
    )]
    NotSameRoomIds(RoomId, RoomId),
}

impl From<RoomError> for RoomServiceError {
    fn from(err: RoomError) -> Self {
        Self::RoomError(err)
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
    ///
    /// Used for providing [`AppContext`] to the newly created [`Room`]s.
    app: AppContext,

    /// Address to [`GracefulShutdown`].
    ///
    /// Use for subscribe newly created [`Room`]s to [`GracefulShutdown`] and
    /// unsubscribe deleted [`Room`]s from [`GracefulShutdown`].
    graceful_shutdown: Addr<GracefulShutdown>,

    /// Path to directory with static [Сontrol API] specs.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    static_specs_dir: String,

    /// Public URL of server. Address for exposed [Client API].
    ///
    /// [Client API]: https://tinyurl.com/yx9thsnr
    public_url: PublicUrl,

    /// [`PeerTrafficWatcher`] for all [`Room`]s of this [`RoomService`].
    peer_traffic_watcher: Arc<dyn PeerTrafficWatcher>,
    /* TODO: Enable in https://github.com/instrumentisto/medea/pull/91
     * /// Service which is responsible for processing [`Peer`]'s metrics
     * received /// from Coturn.
     * ///
     * /// [`Peer`]: crate::media::peer::Peer
     * _coturn_metrics: Addr<CoturnMetricsService>, */
}

impl RoomService {
    /// Creates new [`RoomService`].
    ///
    /// # Errors
    ///
    /// Returns [`RedisError`] if fails to connect to Redis stats server.
    pub fn new(
        room_repo: RoomRepository,
        app: AppContext,
        graceful_shutdown: Addr<GracefulShutdown>,
    ) -> Result<Self, RedisError> {
        let peer_traffic_watcher =
            build_peers_traffic_watcher(&app.config.media);
        Ok(Self {
            // TODO: Enable in https://github.com/instrumentisto/medea/pull/91
            // _coturn_metrics: CoturnMetricsService::new(
            //     &app.config.turn,
            //     peer_traffic_watcher.clone(),
            // )?
            // .start(),
            static_specs_dir: app.config.control.static_specs_dir.clone(),
            public_url: app.config.server.client.http.public_url.clone(),
            peer_traffic_watcher,
            room_repo,
            app,
            graceful_shutdown,
        })
    }

    /// Closes [`Room`] with provided [`RoomId`].
    ///
    /// This is also deletes this [`Room`] from [`RoomRepository`].
    fn close_room(
        &self,
        id: RoomId,
    ) -> LocalBoxFuture<'static, Result<(), MailboxError>> {
        self.room_repo
            .get(&id)
            .map_or(future::ok(()).boxed_local(), |room| {
                shutdown::unsubscribe(
                    &self.graceful_shutdown,
                    room.clone().recipient(),
                    shutdown::Priority(2),
                );

                let room_repo = self.room_repo.clone();
                room.send(Close)
                    .inspect_ok(move |_| room_repo.remove(&id))
                    .boxed_local()
            })
    }

    /// Returns [Control API] sids for `Members` in provided [`RoomSpec`] and
    /// based on `MEDEA_SERVER__CLIENT__HTTP__PUBLIC_URL` config value.
    fn get_sids_from_spec(
        &self,
        spec: &RoomSpec,
    ) -> Result<Sids, RoomServiceError> {
        match spec.members() {
            Ok(members) => Ok(members
                .iter()
                .map(|(member_id, member)| {
                    let sid = Sid::new(
                        self.public_url.clone(),
                        spec.id().clone(),
                        member_id.clone(),
                        member.credentials().clone(),
                    );
                    (member_id.clone(), sid)
                })
                .collect()),
            Err(e) => Err(RoomServiceError::TryFromElement(e)),
        }
    }

    /// Creates a new [`Room`] by the provided [`RoomSpec`].
    ///
    /// # Errors
    ///
    /// With [`RoomServiceError::TryFromElement`] if provided spec is invalid.
    ///
    /// With [`RoomServiceError::RoomAlreadyExists`] if [`Room`] with a provided
    /// ID already exists.
    fn create_room(&self, room_spec: RoomSpec) -> Result<(), RoomServiceError> {
        if self.room_repo.get(&room_spec.id).is_some() {
            return Err(RoomServiceError::RoomAlreadyExists(
                Fid::<ToRoom>::new(room_spec.id),
            ));
        }

        let room_addr = Room::start(
            &room_spec,
            &self.app,
            self.peer_traffic_watcher.clone(),
        )?;

        shutdown::subscribe(
            &self.graceful_shutdown,
            room_addr.clone().recipient(),
            shutdown::Priority(2),
        );

        debug!("New Room [id = {}] started.", room_spec.id);
        self.room_repo.add(room_spec.id, room_addr);

        Ok(())
    }
}

impl Actor for RoomService {
    type Context = Context<Self>;
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
        let room_specs = load_static_specs_from_dir(&self.static_specs_dir)?;

        for spec in room_specs {
            if self.room_repo.contains_room_with_id(spec.id()) {
                return Err(RoomServiceError::RoomAlreadyExists(
                    Fid::<ToRoom>::new(spec.id),
                ));
            }

            let room_id = spec.id().clone();

            let room = Room::start(
                &spec,
                &self.app,
                self.peer_traffic_watcher.clone(),
            )?;

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

/// Type alias for success [`CreateResponse`]'s sids.
///
/// [`CreateResponse`]: medea_control_api_proto::grpc::api::CreateResponse
pub type Sids = HashMap<MemberId, Sid>;

/// Signal for applying [`RoomSpec`] on [`Room`].
#[derive(Message)]
#[rtype(result = "Result<Sids, RoomServiceError>")]
pub struct ApplyRoom {
    /// [`RoomId`] of [`Room`] for which [`RoomSpec`] is provided and should be
    /// applied.
    pub id: RoomId,

    /// [`RoomSpec`] which should be applied on [`Room`].
    pub spec: RoomSpec,
}

impl Handler<ApplyRoom> for RoomService {
    type Result = ResponseFuture<Result<Sids, RoomServiceError>>;

    #[allow(clippy::option_if_let_else)]
    fn handle(
        &mut self,
        msg: ApplyRoom,
        _: &mut Self::Context,
    ) -> Self::Result {
        let sids = match self.get_sids_from_spec(&msg.spec) {
            Ok(sids) => sids,
            Err(err) => {
                return Box::pin(future::err(err));
            }
        };

        if let Some(room) = self.room_repo.get(&msg.id) {
            let fut = room.send(Apply(msg.spec));

            Box::pin(async move {
                fut.await.map_err(RoomServiceError::RoomMailboxErr)??;
                Ok(sids)
            })
        } else {
            let res = self.create_room(msg.spec);

            Box::pin(async move {
                res?;
                Ok(sids)
            })
        }
    }
}

/// Signal for creating new [`Room`].
#[derive(Message)]
#[rtype(result = "Result<Sids, RoomServiceError>")]
pub struct CreateRoom {
    /// [Control API] spec for [`Room`].
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    pub spec: RoomSpec,
}

impl Handler<CreateRoom> for RoomService {
    type Result = Result<Sids, RoomServiceError>;

    fn handle(
        &mut self,
        msg: CreateRoom,
        _: &mut Self::Context,
    ) -> Self::Result {
        let sids = self.get_sids_from_spec(&msg.spec)?;
        self.create_room(msg.spec)?;
        Ok(sids)
    }
}

/// Signal for applying [`MemberSpec`] in [`Room`].
#[derive(Message)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct ApplyMember {
    /// [`Fid`] of [`Member`] for which [`MemberSpec`] is provided and should
    /// be applied.
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    pub fid: Fid<ToMember>,

    /// [`MemberSpec`] which should be applied on [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    pub spec: MemberSpec,
}

impl Handler<ApplyMember> for RoomService {
    type Result = ResponseFuture<Result<(), RoomServiceError>>;

    fn handle(
        &mut self,
        msg: ApplyMember,
        _: &mut Self::Context,
    ) -> Self::Result {
        let (room_id, member_id) = msg.fid.take_all();
        let spec = msg.spec;
        self.room_repo.get(&room_id).map_or_else(
            || {
                future::err(RoomServiceError::RoomNotFound(Fid::<ToRoom>::new(
                    room_id,
                )))
                .boxed_local()
            },
            |room| {
                async move {
                    room.send(crate::signalling::room::ApplyMember(
                        member_id, spec,
                    ))
                    .await
                    .map_err(RoomServiceError::RoomMailboxErr)??;
                    Ok(())
                }
                .boxed_local()
            },
        )
    }
}

/// Signal for create new [`Member`] in [`Room`].
///
/// [`Member`]: crate::signalling::elements::Member
#[derive(Message)]
#[rtype(result = "Result<Sids, RoomServiceError>")]
pub struct CreateMemberInRoom {
    pub id: MemberId,
    pub parent_fid: Fid<ToRoom>,
    pub spec: MemberSpec,
}

impl Handler<CreateMemberInRoom> for RoomService {
    type Result = ResponseFuture<Result<Sids, RoomServiceError>>;

    fn handle(
        &mut self,
        msg: CreateMemberInRoom,
        _: &mut Self::Context,
    ) -> Self::Result {
        let room_id = msg.parent_fid.take_room_id();
        let member_id = msg.id;
        let spec = msg.spec;
        let sid = Sid::new(
            self.public_url.clone(),
            room_id.clone(),
            member_id.clone(),
            spec.credentials().clone(),
        );

        self.room_repo.get(&room_id).map_or_else(
            || {
                future::err(RoomServiceError::RoomNotFound(Fid::<ToRoom>::new(
                    room_id,
                )))
                .boxed_local()
            },
            |room| {
                async move {
                    room.send(CreateMember(member_id.clone(), spec))
                        .await
                        .map_err(RoomServiceError::RoomMailboxErr)??;
                    Ok(hashmap! {member_id => sid})
                }
                .boxed_local()
            },
        )
    }
}

/// Signal for create new [`Endpoint`] in [`Room`].
///
/// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
#[derive(Message)]
#[rtype(result = "Result<Sids, RoomServiceError>")]
pub struct CreateEndpointInRoom {
    pub id: EndpointId,
    pub parent_fid: Fid<ToMember>,
    pub spec: EndpointSpec,
}

impl Handler<CreateEndpointInRoom> for RoomService {
    type Result = ResponseFuture<Result<Sids, RoomServiceError>>;

    fn handle(
        &mut self,
        msg: CreateEndpointInRoom,
        _: &mut Self::Context,
    ) -> Self::Result {
        let (room_id, member_id) = msg.parent_fid.take_all();
        let endpoint_id = msg.id;
        let spec = msg.spec;

        self.room_repo.get(&room_id).map_or_else(
            || {
                future::err(RoomServiceError::RoomNotFound(Fid::<ToRoom>::new(
                    room_id,
                )))
                .boxed_local()
            },
            |room| {
                async move {
                    room.send(CreateEndpoint {
                        member_id,
                        endpoint_id,
                        spec,
                    })
                    .await
                    .map_err(RoomServiceError::RoomMailboxErr)??;
                    Ok(HashMap::new())
                }
                .boxed_local()
            },
        )
    }
}

/// State which indicates that [`DeleteElements`] message was validated and can
/// be send to [`RoomService`].
pub struct Validated;

/// State which indicates that [`DeleteElements`] message is unvalidated and
/// should be validated with `validate()` function of [`DeleteElements`] in
/// [`Unvalidated`] state before sending to [`RoomService`].
pub struct Unvalidated;

// Clippy lint show use_self errors for DeleteElements with generic state. This
// is fix for it. This allow not works on function.
impl DeleteElements<Unvalidated> {
    /// Creates new [`DeleteElements`] in [`Unvalidated`] state.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            fids: Vec::new(),
            _validation_state: PhantomData,
        }
    }

    /// Adds [`StatefulFid`] to request.
    #[inline]
    pub fn add_fid(&mut self, fid: StatefulFid) {
        self.fids.push(fid)
    }

    /// Validates request. It must have at least one fid, all fids must share
    /// same [`RoomId`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomServiceError::EmptyUrisList`] if [`DeleteElements`]
    /// consists of an empty [`Vec`] of [`StatefulFid`]s.
    pub fn validate(
        self,
    ) -> Result<DeleteElements<Validated>, RoomServiceError> {
        if self.fids.is_empty() {
            return Err(RoomServiceError::EmptyUrisList);
        }

        let first_room_id = self.fids[0].room_id();

        for id in &self.fids {
            if first_room_id != id.room_id() {
                return Err(RoomServiceError::NotSameRoomIds(
                    first_room_id.clone(),
                    id.room_id().clone(),
                ));
            }
        }

        Ok(DeleteElements {
            fids: self.fids,
            _validation_state: PhantomData,
        })
    }
}

/// Signal for delete [Control API] elements.
///
/// This message can be in two states: [`Validated`] and [`Unvalidated`].
///
/// For ability to send this message to [`RoomService`] [`DeleteElements`]
/// should be in [`Validated`] state. You can go to [`Validated`] state
/// from [`Unvalidated`] with [`DeleteElements::validate`] function
/// which will validate all [`StatefulFid`]s.
///
/// Validation doesn't guarantee that message can't return [`RoomServiceError`].
/// This is just validation for errors which we can catch before sending
/// message.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Message, Default)]
#[rtype(result = "Result<(), RoomServiceError>")]
pub struct DeleteElements<T> {
    fids: Vec<StatefulFid>,
    _validation_state: PhantomData<T>,
}

impl Handler<DeleteElements<Validated>> for RoomService {
    type Result = ResponseFuture<Result<(), RoomServiceError>>;

    // TODO: delete 'clippy::unnecessary_filter_map` when drain_filter TODO will
    //       be resolved.
    #[allow(clippy::if_not_else, clippy::unnecessary_filter_map)]
    fn handle(
        &mut self,
        msg: DeleteElements<Validated>,
        _: &mut Self::Context,
    ) -> Self::Result {
        let mut deletes_from_room: Vec<StatefulFid> = Vec::new();

        // TODO: use Vec::drain_filter when it will be in stable
        let room_messages_futs: Vec<ResponseFuture<Result<(), MailboxError>>> =
            msg.fids
                .into_iter()
                .filter_map(|fid| {
                    if let StatefulFid::Room(room_id) = fid {
                        Some(self.close_room(room_id.take_room_id()))
                    } else {
                        deletes_from_room.push(fid);
                        None
                    }
                })
                .collect();

        if !room_messages_futs.is_empty() {
            future::try_join_all(room_messages_futs)
                .map_ok(drop)
                .map_err(RoomServiceError::RoomMailboxErr)
                .boxed_local()
        } else if !deletes_from_room.is_empty() {
            let room_id = deletes_from_room[0].room_id().clone();

            self.room_repo.get(&room_id).map_or_else(
                || future::ok(()).boxed_local(),
                |room| {
                    room.send(Delete(deletes_from_room))
                        .map_ok(drop)
                        .map_err(RoomServiceError::RoomMailboxErr)
                        .err_into()
                        .boxed_local()
                },
            )
        } else {
            future::err(RoomServiceError::EmptyUrisList).boxed_local()
        }
    }
}

/// Serialized to protobuf `Element`s which will be returned from [`Get`] on
/// success result.
type SerializedElements = HashMap<StatefulFid, proto::Element>;

/// Message which returns serialized to protobuf objects by provided
/// [`Fid`].
#[derive(Message)]
#[rtype(result = "Result<SerializedElements, RoomServiceError>")]
pub struct Get(pub Vec<StatefulFid>);

impl Handler<Get> for RoomService {
    type Result = ResponseFuture<Result<SerializedElements, RoomServiceError>>;

    fn handle(&mut self, msg: Get, _: &mut Self::Context) -> Self::Result {
        let mut rooms_elements = HashMap::new();
        for fid in msg.0 {
            let room_id = fid.room_id();

            if let Some(room) = self.room_repo.get(room_id) {
                rooms_elements
                    .entry(room)
                    .or_insert_with(Vec::new)
                    .push(fid);
            } else {
                return future::err(RoomServiceError::RoomNotFound(fid.into()))
                    .boxed_local();
            }
        }

        let mut futs = Vec::new();
        for (room, elements) in rooms_elements {
            futs.push(room.send(SerializeProto(elements)));
        }

        async {
            let results = future::try_join_all(futs)
                .await
                .map_err(RoomServiceError::RoomMailboxErr)?;

            let mut all = HashMap::new();
            for res in results {
                match res {
                    Ok(r) => all.extend(r),
                    Err(e) => return Err(RoomServiceError::from(e)),
                }
            }
            Ok(all)
        }
        .boxed_local()
    }
}

#[cfg(test)]
mod delete_elements_validation_specs {
    use std::convert::TryFrom as _;

    use super::*;

    #[test]
    fn empty_fids_list() {
        let elements = DeleteElements::new();
        match elements.validate() {
            Ok(_) => panic!(
                "Validation should fail with EmptyUrisList but returned Ok."
            ),
            Err(e) => match e {
                RoomServiceError::EmptyUrisList => (),
                _ => panic!(
                    "Validation should fail with EmptyList error but errored \
                     with {:?}.",
                    e
                ),
            },
        }
    }

    #[test]
    fn error_if_not_same_room_ids() {
        let mut elements = DeleteElements::new();
        ["room_id/member", "another_room_id/member"]
            .iter()
            .map(|fid| StatefulFid::try_from((*fid).to_string()).unwrap())
            .for_each(|fid| elements.add_fid(fid));

        match elements.validate() {
            Ok(_) => panic!(
                "Validation should fail with NotSameRoomIds but returned Ok."
            ),
            Err(e) => match e {
                RoomServiceError::NotSameRoomIds(first, another) => {
                    assert_eq!(&first.to_string(), "room_id");
                    assert_eq!(&another.to_string(), "another_room_id");
                }
                _ => panic!(
                    "Validation should fail with NotSameRoomIds error but \
                     errored with {:?}.",
                    e
                ),
            },
        }
    }

    #[test]
    fn success_if_all_ok() {
        let mut elements = DeleteElements::new();
        [
            "room_id/member_id",
            "room_id/another_member_id",
            "room_id/member_id/endpoint_id",
        ]
        .iter()
        .map(|fid| StatefulFid::try_from((*fid).to_string()).unwrap())
        .for_each(|fid| elements.add_fid(fid));

        assert!(elements.validate().is_ok());
    }
}

#[cfg(test)]
mod room_service_specs {
    use std::convert::TryFrom as _;

    use crate::{
        api::control::{
            endpoints::{
                webrtc_publish_endpoint::{
                    AudioSettings, P2pMode, VideoSettings,
                },
                WebRtcPublishEndpoint,
            },
            member::{Credential, MemberElement},
            pipeline::Pipeline,
            refs::{Fid, ToEndpoint},
            RoomElement, RootElement, WebRtcPublishId,
        },
        conf::{self, Conf},
    };

    use super::*;

    /// Returns [`RoomSpec`] parsed from
    /// `../../tests/specs/pub-sub-video-call.yml` file.
    ///
    /// Note that YAML spec is loads on compile time with [`include_str`]
    /// macro.
    fn room_spec() -> RoomSpec {
        const ROOM_SPEC: &str =
            include_str!("../../tests/specs/pub-sub-video-call.yml");

        let parsed: RootElement = serde_yaml::from_str(ROOM_SPEC).unwrap();
        RoomSpec::try_from(&parsed).unwrap()
    }

    /// Returns [`AppContext`] with default [`Conf`] and mocked
    /// [`TurnAuthService`].
    fn app_ctx() -> AppContext {
        let turn_service = crate::turn::new_turn_auth_service_mock();
        AppContext::new(Conf::default(), turn_service)
    }

    /// Returns [`Addr`] to [`RoomService`].
    fn room_service(room_repo: RoomRepository) -> Addr<RoomService> {
        let conf = Conf::default();
        let shutdown_timeout = conf.shutdown.timeout;

        let app = app_ctx();
        let graceful_shutdown = GracefulShutdown::new(shutdown_timeout).start();

        RoomService::new(room_repo, app, graceful_shutdown)
            .unwrap()
            .start()
    }

    /// Returns [`Future`] used for testing of all create methods of
    /// [`RoomService`].
    ///
    /// This macro automatically stops [`actix::System`] when test completed.
    ///
    /// `$room_service` - [`Addr`] to [`RoomService`],
    ///
    /// `$create_msg` - [`actix::Message`] which will create `Element`,
    ///
    /// `$element_fid` - [`StatefulFid`] to `Element` which you try to
    /// create,
    ///
    /// `$test` - closure in which will be provided created
    /// [`Element`].
    macro_rules! test_for_create {
        (
            $room_service:expr,
            $create_msg:expr,
            $element_fid:expr,
            $test:expr
        ) => {
            async move {
                let get_msg = Get(vec![$element_fid.clone()]);
                $room_service.send($create_msg).await.unwrap().unwrap();
                let mut resp =
                    $room_service.send(get_msg).await.unwrap().unwrap();
                let el = resp.remove(&$element_fid).unwrap().el.unwrap();
                $test(el);
            }
        };
    }

    #[actix_rt::test]
    async fn create_room() {
        let room_service = room_service(RoomRepository::new());
        let spec = room_spec();
        let caller_fid =
            StatefulFid::try_from("pub-sub-video-call/caller".to_string())
                .unwrap();

        test_for_create!(
            room_service,
            CreateRoom { spec },
            caller_fid,
            |member_el| {
                match member_el {
                    proto::element::El::Member(member) => {
                        assert_eq!(member.pipeline.len(), 1);
                    }
                    _ => unreachable!(),
                }
            }
        )
        .await;
    }

    #[actix_rt::test]
    async fn create_member() {
        let spec = room_spec();
        let member_spec = spec
            .members()
            .unwrap()
            .get(&MemberId::from("caller"))
            .unwrap()
            .clone();

        let room_id = RoomId::from("pub-sub-video-call");
        let room = Room::start(
            &spec,
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap();
        let room_service = room_service(RoomRepository::from(hashmap!(
            room_id.clone() => room,
        )));

        let member_parent_fid = Fid::<ToRoom>::new(room_id);
        let member_id = MemberId::from("test-member");
        let member_full_id: StatefulFid = member_parent_fid
            .clone()
            .push_member_id(member_id.clone())
            .into();

        test_for_create!(
            room_service,
            CreateMemberInRoom {
                id: member_id,
                spec: member_spec,
                parent_fid: member_parent_fid,
            },
            member_full_id,
            |member_el| {
                match member_el {
                    proto::element::El::Member(member) => {
                        assert_eq!(member.pipeline.len(), 1);
                    }
                    _ => unreachable!(),
                }
            }
        )
        .await;
    }

    #[actix_rt::test]
    async fn create_endpoint() {
        let spec = room_spec();

        let mut endpoint_spec = spec
            .members()
            .unwrap()
            .get(&"caller".to_string().into())
            .unwrap()
            .get_publish_endpoint_by_id(WebRtcPublishId::from(String::from(
                "publish",
            )))
            .unwrap()
            .clone();
        endpoint_spec.p2p = P2pMode::Never;
        let endpoint_spec = endpoint_spec.into();

        let room_id = RoomId::from("pub-sub-video-call");
        let room = Room::start(
            &spec,
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap();
        let room_service = room_service(RoomRepository::from(hashmap!(
            room_id.clone() => room,
        )));

        let endpoint_parent_fid =
            Fid::<ToMember>::new(room_id, MemberId::from("caller"));
        let endpoint_id = EndpointId::from(String::from("test-publish"));
        let endpoint_full_id: StatefulFid = endpoint_parent_fid
            .clone()
            .push_endpoint_id(endpoint_id.clone())
            .into();

        test_for_create!(
            room_service,
            CreateEndpointInRoom {
                id: endpoint_id,
                spec: endpoint_spec,
                parent_fid: endpoint_parent_fid,
            },
            endpoint_full_id,
            |endpoint_el| {
                match endpoint_el {
                    proto::element::El::WebrtcPub(publish) => {
                        let endpoint = WebRtcPublishEndpoint::from(&publish);
                        assert_eq!(endpoint.p2p, P2pMode::Never);
                    }
                    _ => unreachable!(),
                }
            }
        )
        .await;
    }

    /// Returns [`Future`] used for testing of all delete/get methods of
    /// [`RoomService`].
    ///
    /// This test is simply try to delete element with provided
    /// [`StatefulFid`] and the try to get it. If result of getting
    /// deleted element is error then test considers successful.
    ///
    /// This function automatically stops [`actix::System`] when test completed.
    async fn test_for_delete_and_get(
        room_service: Addr<RoomService>,
        element_fid: StatefulFid,
    ) {
        let mut delete_msg = DeleteElements::new();
        delete_msg.add_fid(element_fid.clone());
        let delete_msg = delete_msg.validate().unwrap();

        room_service.send(delete_msg).await.unwrap().unwrap();
        let get_result =
            room_service.send(Get(vec![element_fid])).await.unwrap();

        assert!(get_result.is_err());
    }

    #[actix_rt::test]
    async fn delete_and_get_room() {
        let room_id = RoomId::from("pub-sub-video-call");
        let room_full_id =
            StatefulFid::from(Fid::<ToRoom>::new(room_id.clone()));

        let room = Room::start(
            &room_spec(),
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap();
        let room_service = room_service(RoomRepository::from(hashmap!(
            room_id => room,
        )));

        test_for_delete_and_get(room_service, room_full_id).await;
    }

    #[actix_rt::test]
    async fn delete_and_get_member() {
        let room_id = RoomId::from("pub-sub-video-call");
        let member_fid = StatefulFid::from(Fid::<ToMember>::new(
            room_id.clone(),
            MemberId::from("caller"),
        ));

        let room = Room::start(
            &room_spec(),
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap();
        let room_service = room_service(RoomRepository::from(hashmap!(
            room_id => room,
        )));

        test_for_delete_and_get(room_service, member_fid).await;
    }

    #[actix_rt::test]
    async fn delete_and_get_endpoint() {
        let room_id = RoomId::from("pub-sub-video-call");
        let endpoint_fid = StatefulFid::from(Fid::<ToEndpoint>::new(
            room_id.clone(),
            MemberId::from("caller"),
            EndpointId::from(String::from("publish")),
        ));

        let room = Room::start(
            &room_spec(),
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap();
        let room_service = room_service(RoomRepository::from(hashmap!(
            room_id => room,
        )));

        test_for_delete_and_get(room_service, endpoint_fid).await;
    }

    #[actix_rt::test]
    async fn create_member_via_apply_room() {
        let room_id = RoomId::from("test");
        let room = Room::start(
            &RoomSpec {
                id: room_id.clone(),
                pipeline: Pipeline::new(HashMap::new()),
            },
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap();
        let room_service = room_service(RoomRepository::from(hashmap!(
            room_id => room,
        )));

        let mut apply_result = room_service
            .send(ApplyRoom {
                id: RoomId::from("test"),
                spec: RoomSpec {
                    id: RoomId::from("test"),
                    pipeline: Pipeline::new(hashmap! {
                        MemberId::from("member1") => RoomElement::Member {
                            spec: Pipeline::new(hashmap! {
                                EndpointId::from(String::from("pub")) =>
                                    MemberElement::WebRtcPublishEndpoint {
                                        spec: WebRtcPublishEndpoint {
                                            p2p: P2pMode::Always,
                                            force_relay: false,
                                            audio_settings:
                                                AudioSettings::default(),
                                            video_settings:
                                                VideoSettings::default()
                                        }
                                    }
                            }),
                            credentials: Credential::Plain(String::from("1")),
                            on_leave: None,
                            on_join: None,
                            idle_timeout: None,
                            reconnect_timeout: None,
                            ping_interval: None,
                        },
                        MemberId::from("member2") => RoomElement::Member {
                            spec: Pipeline::new(HashMap::new()),
                            credentials: Credential::Plain(String::from("2")),
                            on_leave: None,
                            on_join: None,
                            idle_timeout: None,
                            reconnect_timeout: None,
                            ping_interval: None,
                        }
                    }),
                },
            })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(apply_result.len(), 2);
        let member1_sid =
            apply_result.remove(&MemberId::from("member1")).unwrap();
        let member2_sid =
            apply_result.remove(&MemberId::from("member2")).unwrap();

        assert_eq!(
            member1_sid.to_string(),
            "ws://127.0.0.1:8080/ws/test/member1?token=1"
        );
        assert_eq!(
            member2_sid.to_string(),
            "ws://127.0.0.1:8080/ws/test/member2?token=2"
        );

        let mut get_resp: HashMap<StatefulFid, proto::Element> = room_service
            .send(Get(vec![
                StatefulFid::try_from(String::from("test")).unwrap()
            ]))
            .await
            .unwrap()
            .unwrap();

        // panic!("{:#?}", get_resp.keys());
        let room = get_resp
            .remove(&StatefulFid::try_from(String::from("test")).unwrap())
            .unwrap();
        match room.el.unwrap() {
            proto::element::El::Room(mut room) => {
                let member1 = room
                    .pipeline
                    .remove("member1")
                    .unwrap()
                    .el
                    .map(|el| match el {
                        proto::room::element::El::Member(member) => {
                            MemberSpec::try_from(member).unwrap()
                        }
                        _ => unreachable!(),
                    })
                    .unwrap();
                let member2 = room
                    .pipeline
                    .remove("member2")
                    .unwrap()
                    .el
                    .map(|el| match el {
                        proto::room::element::El::Member(member) => {
                            MemberSpec::try_from(member).unwrap()
                        }
                        _ => unreachable!(),
                    })
                    .unwrap();

                assert_eq!(member1.publish_endpoints().count(), 1);
                assert_eq!(member1.play_endpoints().count(), 0);

                assert_eq!(member2.publish_endpoints().count(), 0);
                assert_eq!(member2.play_endpoints().count(), 0);
            }
            _ => unreachable!(),
        };
    }

    #[actix_rt::test]
    async fn create_endpoints_via_apply_member() {
        let room_id = RoomId::from("test");
        let member_id = MemberId::from("member1");
        let room = Room::start(
            &RoomSpec {
                id: room_id.clone(),
                pipeline: Pipeline::new(HashMap::new()),
            },
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap();

        let room_service = room_service(RoomRepository::from(hashmap!(
            room_id.clone() => room,
        )));

        // create member without endpoints
        room_service
            .send(CreateMemberInRoom {
                id: member_id.clone(),
                parent_fid: Fid::<ToRoom>::new(room_id.clone()),
                spec: MemberSpec::new(
                    Pipeline::new(HashMap::new()),
                    Credential::Plain(String::from("asd")),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
            })
            .await
            .unwrap()
            .unwrap();

        // add two publish endpoints
        room_service
            .send(ApplyMember {
                fid: Fid::<ToMember>::new(room_id, member_id),
                spec: MemberSpec::new(
                    Pipeline::new(hashmap! {
                        EndpointId::from(String::from("pub1")) =>
                            MemberElement::WebRtcPublishEndpoint {
                            spec: WebRtcPublishEndpoint {
                                p2p: P2pMode::Always,
                                force_relay: false,
                                audio_settings: AudioSettings::default(),
                                video_settings: VideoSettings::default()
                            }
                        },
                        EndpointId::from(String::from("pub2")) =>
                            MemberElement::WebRtcPublishEndpoint {
                            spec: WebRtcPublishEndpoint {
                                p2p: P2pMode::Always,
                                force_relay: false,
                                audio_settings: AudioSettings::default(),
                                video_settings: VideoSettings::default()
                            }
                        },
                    }),
                    Credential::Plain(String::from("asd")),
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
            })
            .await
            .unwrap()
            .unwrap();

        let resp = room_service
            .send(Get(vec![
                StatefulFid::try_from(String::from("test/member1/pub1"))
                    .unwrap(),
                StatefulFid::try_from(String::from("test/member1/pub2"))
                    .unwrap(),
            ]))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(resp.len(), 2);
    }

    #[actix_rt::test]
    async fn delete_members_via_apply_room() {
        let room_id = RoomId::from("pub-sub-video-call");
        let room = Room::start(
            &room_spec(),
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap();
        let room_service = room_service(RoomRepository::from(hashmap!(
            room_id.clone() => room,
        )));

        room_service
            .send(ApplyRoom {
                id: room_id.clone(),
                spec: RoomSpec {
                    id: room_id.clone(),
                    pipeline: Pipeline::new(HashMap::new()),
                },
            })
            .await
            .unwrap()
            .unwrap();

        let room = room_service
            .send(Get(vec![StatefulFid::try_from(String::from(
                "pub-sub-video-call",
            ))
            .unwrap()]))
            .await
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|(_, room)| room.el.unwrap())
            .next()
            .unwrap();

        match room {
            proto::element::El::Room(room) => {
                // make sure members are deleted
                assert!(room.pipeline.is_empty())
            }
            _ => unreachable!(),
        }
    }

    #[actix_rt::test]
    async fn delete_endpoints_via_apply_room() {
        let room_id = RoomId::from("pub-sub-video-call");
        let room = Room::start(
            &room_spec(),
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap();
        let room_service = room_service(RoomRepository::from(hashmap!(
            room_id.clone() => room,
        )));

        room_service
            .send(ApplyRoom {
                id: room_id.clone(),
                spec: RoomSpec {
                    id: room_id.clone(),
                    pipeline: Pipeline::new(hashmap! {
                        MemberId::from("caller") => RoomElement::Member {
                            spec: Pipeline::new(HashMap::new()),
                            credentials:
                                Credential::Plain(String::from("test")),
                            on_leave: None,
                            on_join: None,
                            idle_timeout: None,
                            reconnect_timeout: None,
                            ping_interval: None,
                        },
                        MemberId::from("responder") => RoomElement::Member {
                            spec: Pipeline::new(HashMap::new()),
                            credentials:
                                Credential::Plain(String::from("test")),
                            on_leave: None,
                            on_join: None,
                            idle_timeout: None,
                            reconnect_timeout: None,
                            ping_interval: None,
                        }
                    }),
                },
            })
            .await
            .unwrap()
            .unwrap();

        let room = room_service
            .send(Get(vec![StatefulFid::try_from(String::from(
                "pub-sub-video-call",
            ))
            .unwrap()]))
            .await
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|(_, room)| room.el.unwrap())
            .next()
            .unwrap();

        match room {
            proto::element::El::Room(mut room) => {
                assert_eq!(room.pipeline.len(), 2);
                let caller = room
                    .pipeline
                    .remove("caller")
                    .unwrap()
                    .el
                    .map(|el| match el {
                        proto::room::element::El::Member(member) => {
                            MemberSpec::try_from(member).unwrap()
                        }
                        _ => unreachable!(),
                    })
                    .unwrap();
                let responder = room
                    .pipeline
                    .remove("responder")
                    .unwrap()
                    .el
                    .map(|el| match el {
                        proto::room::element::El::Member(member) => {
                            MemberSpec::try_from(member).unwrap()
                        }
                        _ => unreachable!(),
                    })
                    .unwrap();

                assert_eq!(caller.play_endpoints().count(), 0);
                assert_eq!(caller.publish_endpoints().count(), 0);
                assert_eq!(responder.play_endpoints().count(), 0);
                assert_eq!(responder.publish_endpoints().count(), 0);
            }
            _ => unreachable!(),
        };
    }
}
