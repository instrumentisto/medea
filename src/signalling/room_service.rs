//! Service which provides CRUD actions for [`Room`].

use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use actix::{
    Actor, Addr, Context, Handler, MailboxError, Message, ResponseFuture,
};
use derive_more::Display;
use failure::Fail;
use futures::{
    future::{self, LocalBoxFuture},
    FutureExt as _, TryFutureExt as _,
};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::control::{
        endpoints::EndpointSpec,
        load_static_specs_from_dir,
        refs::{Fid, StatefulFid, ToMember, ToRoom},
        EndpointId, LoadStaticControlSpecsError, MemberId, MemberSpec, RoomId,
        RoomSpec, TryFromElementError,
    },
    log::prelude::*,
    shutdown::{self, GracefulShutdown},
    signalling::{
        peers::{build_peers_traffic_watcher, PeerTrafficWatcher},
        room::{
            Close, CreateEndpoint, CreateMember, Delete, RoomError,
            SerializeProto,
        },
        room_repo::RoomRepository,
        Room,
    },
    turn::coturn_metrics::CoturnMetricsService,
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

    /// Wrapper for the [`PeerTrafficWatcher`] [`MailboxError`].
    #[display(fmt = "TrafficWatcher mailbox error: {:?}", _0)]
    TrafficWatcherMailbox(MailboxError),

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
    public_url: String,

    /// [`PeerTrafficWatcher`] for all [`Room`]s of this [`RoomService`].
    peer_traffic_watcher: Arc<dyn PeerTrafficWatcher>,

    /// Service which is responsible for processing [`PeerConnection`]'s
    /// metrics received from the Coturn.
    _coturn_metrics: Addr<CoturnMetricsService>,
}

impl RoomService {
    /// Creates new [`RoomService`].
    ///
    /// # Errors
    ///
    /// Returns [`redis_pub_sub::RedisError`] if [`CoturnMetricsService`] fails
    /// to connect to Redis stats server.
    pub fn new(
        room_repo: RoomRepository,
        app: AppContext,
        graceful_shutdown: Addr<GracefulShutdown>,
    ) -> Result<Self, redis_pub_sub::RedisError> {
        let peer_traffic_watcher =
            build_peers_traffic_watcher(&app.config.media);
        Ok(Self {
            _coturn_metrics: CoturnMetricsService::new(
                &app.config.turn,
                peer_traffic_watcher.clone(),
            )?
            .start(),
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
        if let Some(room) = self.room_repo.get(&id) {
            self.peer_traffic_watcher.unregister_room(id.clone());
            shutdown::unsubscribe(
                &self.graceful_shutdown,
                room.clone().recipient(),
                shutdown::Priority(2),
            );

            let room_repo = self.room_repo.clone();
            let sending = room.send(Close);
            async move {
                let res = sending.await;
                if res.is_ok() {
                    room_repo.remove(&id);
                }
                res
            }
            .boxed_local()
        } else {
            async { Ok(()) }.boxed_local()
        }
    }

    /// Creates [`Room`] based on provided [`RoomSpec`].
    ///
    /// Subscribes this [`Room`] to the [`GracefulShutdown`], and registers as
    /// [`Peer`] stats listener to the [`PeersTrafficWatcher`].
    fn create_room(
        &self,
        spec: RoomSpec,
    ) -> LocalBoxFuture<'static, Result<(), RoomServiceError>> {
        if self.room_repo.contains_room_with_id(spec.id()) {
            return future::err(RoomServiceError::RoomAlreadyExists(Fid::<
                ToRoom,
            >::new(
                spec.id
            )))
            .boxed_local();
        }

        let room = match Room::new(
            &spec,
            &self.app,
            self.peer_traffic_watcher.clone(),
        ) {
            Ok(room) => room.start(),
            Err(err) => {
                return future::err(RoomServiceError::RoomError(err))
                    .boxed_local();
            }
        };

        let graceful_shutdown = self.graceful_shutdown.clone();
        let peer_traffic_watcher = self.peer_traffic_watcher.clone();
        let room_repo = self.room_repo.clone();
        async move {
            peer_traffic_watcher
                .register_room(spec.id().clone(), Box::new(room.downgrade()))
                .await
                .map_err(RoomServiceError::TrafficWatcherMailbox)?;
            shutdown::subscribe(
                &graceful_shutdown,
                room.clone().recipient(),
                shutdown::Priority(2),
            );

            room_repo.add(spec.id().clone(), room);
            debug!("New Room [id = {}] started.", spec.id());
            Ok(())
        }
        .boxed_local()
    }

    /// Returns [Control API] sid based on provided arguments and
    /// `MEDEA_SERVER__CLIENT__HTTP__PUBLIC_URL` config value.
    fn get_sid(
        &self,
        room_id: &RoomId,
        member_id: &MemberId,
        credentials: &str,
    ) -> String {
        format!(
            "{}/{}/{}/{}",
            self.public_url, room_id, member_id, credentials
        )
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
    type Result = ResponseFuture<Result<(), RoomServiceError>>;

    fn handle(
        &mut self,
        _: StartStaticRooms,
        _: &mut Self::Context,
    ) -> Self::Result {
        let room_specs =
            match load_static_specs_from_dir(&self.static_specs_dir) {
                Ok(specs) => specs,
                Err(err) => {
                    return future::err(
                        RoomServiceError::FailedToLoadStaticSpecs(err),
                    )
                    .boxed_local()
                }
            };

        future::try_join_all(
            room_specs.into_iter().map(|spec| self.create_room(spec)),
        )
        .map_ok(|_| ())
        .boxed_local()
    }
}

/// Type alias for success [`CreateResponse`]'s sids.
pub type Sids = HashMap<String, String>;

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
    type Result = ResponseFuture<Result<Sids, RoomServiceError>>;

    fn handle(
        &mut self,
        msg: CreateRoom,
        _: &mut Self::Context,
    ) -> Self::Result {
        let room_spec = msg.spec;
        let sid = match room_spec.members() {
            Ok(members) => members
                .iter()
                .map(|(member_id, member)| {
                    let uri = self.get_sid(
                        room_spec.id(),
                        &member_id,
                        member.credentials(),
                    );
                    (member_id.clone().to_string(), uri)
                })
                .collect(),
            Err(e) => {
                return future::err(RoomServiceError::TryFromElement(e))
                    .boxed_local()
            }
        };

        self.create_room(room_spec).map_ok(|_| sid).boxed_local()
    }
}

/// Signal for create new [`Member`] in [`Room`].
///
/// [`Member`]: crate::signalling::elements::member::Member
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
        let sid = self.get_sid(&room_id, &msg.id, msg.spec.credentials());
        let mut sids = HashMap::new();
        sids.insert(msg.id.to_string(), sid);

        if let Some(room) = self.room_repo.get(&room_id) {
            let sending = room.send(CreateMember(msg.id, msg.spec));
            async {
                sending.await.map_err(RoomServiceError::RoomMailboxErr)??;
                Ok(sids)
            }
            .boxed_local()
        } else {
            future::err(RoomServiceError::RoomNotFound(Fid::<ToRoom>::new(
                room_id,
            )))
            .boxed_local()
        }
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

        if let Some(room) = self.room_repo.get(&room_id) {
            let sending = room.send(CreateEndpoint {
                member_id,
                endpoint_id,
                spec: msg.spec,
            });
            async {
                sending.await.map_err(RoomServiceError::RoomMailboxErr)??;
                Ok(HashMap::new())
            }
            .boxed_local()
        } else {
            future::err(RoomServiceError::RoomNotFound(Fid::<ToRoom>::new(
                room_id,
            )))
            .boxed_local()
        }
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
#[allow(clippy::use_self)]
impl DeleteElements<Unvalidated> {
    /// Creates new [`DeleteElements`] in [`Unvalidated`] state.
    pub fn new() -> Self {
        Self {
            fids: Vec::new(),
            _validation_state: PhantomData,
        }
    }

    /// Adds [`StatefulFid`] to request.
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
                .map_ok(|_| ())
                .map_err(RoomServiceError::RoomMailboxErr)
                .boxed_local()
        } else if !deletes_from_room.is_empty() {
            let room_id = deletes_from_room[0].room_id().clone();

            if let Some(room) = self.room_repo.get(&room_id) {
                let sending = room.send(Delete(deletes_from_room));
                async {
                    sending.await.map_err(RoomServiceError::RoomMailboxErr)?;
                    Ok(())
                }
                .boxed_local()
            } else {
                future::ok(()).boxed_local()
            }
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
            endpoints::webrtc_publish_endpoint::P2pMode,
            refs::{Fid, ToEndpoint},
            RootElement, WebRtcPublishId,
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
                resp.remove(&$element_fid).unwrap();
                actix::System::current().stop();
            }
        };
    }

    #[actix_rt::test]
    async fn create_room() {
        let room_service = room_service(RoomRepository::new(HashMap::new()));
        let spec = room_spec();
        let caller_fid =
            StatefulFid::try_from("pub-sub-video-call/caller".to_string())
                .unwrap();

        test_for_create!(
            room_service,
            CreateRoom { spec },
            caller_fid,
            |member_el| {
                assert_eq!(member_el.get_member().get_pipeline().len(), 1);
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
            .get(&"caller".to_string().into())
            .unwrap()
            .clone();

        let room_id = RoomId::from("pub-sub-video-call");
        let room = Room::new(
            &spec,
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap()
        .start();
        let room_service = room_service(RoomRepository::new(hashmap!(
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
                assert_eq!(member_el.get_member().get_pipeline().len(), 1);
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
            .get_publish_endpoint_by_id(WebRtcPublishId::from("publish"))
            .unwrap()
            .clone();
        endpoint_spec.p2p = P2pMode::Never;
        let endpoint_spec = endpoint_spec.into();

        let room_id = RoomId::from("pub-sub-video-call");
        let room = Room::new(
            &spec,
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap()
        .start();
        let room_service = room_service(RoomRepository::new(hashmap!(
            room_id.clone() => room,
        )));

        let endpoint_parent_fid =
            Fid::<ToMember>::new(room_id, MemberId::from("caller"));
        let endpoint_id = EndpointId::from("test-publish");
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
                assert_eq!(
                    endpoint_el.get_webrtc_pub().get_p2p(),
                    P2pMode::Never.into()
                );
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

        actix::System::current().stop();
    }

    #[actix_rt::test]
    async fn delete_and_get_room() {
        let room_id = RoomId::from("pub-sub-video-call");
        let room_full_id =
            StatefulFid::from(Fid::<ToRoom>::new(room_id.clone()));

        let room = Room::new(
            &room_spec(),
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap()
        .start();
        let room_service = room_service(RoomRepository::new(hashmap!(
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

        let room = Room::new(
            &room_spec(),
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap()
        .start();
        let room_service = room_service(RoomRepository::new(hashmap!(
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
            EndpointId::from("publish"),
        ));

        let room = Room::new(
            &room_spec(),
            &app_ctx(),
            build_peers_traffic_watcher(&conf::Media::default()),
        )
        .unwrap()
        .start();
        let room_service = room_service(RoomRepository::new(hashmap!(
            room_id => room,
        )));

        test_for_delete_and_get(room_service, endpoint_fid).await;
    }
}
