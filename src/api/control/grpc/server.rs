//! Implementation of gRPC control API.

// Fix clippy needless_return in macro.
#![allow(clippy::needless_return)]

use std::{collections::HashMap, convert::TryFrom, sync::Arc};

use actix::{
    Actor, Addr, Arbiter, Context, Handler, MailboxError, ResponseFuture,
};
use derive_more::Display;
use failure::Fail;
use futures::future::{Either, Future};
use grpcio::{
    Environment, RpcContext, RpcStatus, RpcStatusCode, Server, ServerBuilder,
    UnarySink,
};
use medea_grpc_proto::{
    control::{
        ApplyRequest, CreateRequest, CreateResponse, Error, GetResponse,
        IdRequest, Response,
    },
    control_grpc::{create_control_api, ControlApi},
};

use crate::{
    api::{
        control::{
            local_uri::{
                IsEndpointId, IsMemberId, IsRoomId, LocalUri,
                LocalUriParseError, StatefulLocalUri,
            },
            Endpoint, MemberId, MemberSpec, RoomId, RoomSpec,
            TryFromElementError, TryFromProtobufError,
        },
        error_codes::{ErrorCode, ErrorResponse},
    },
    log::prelude::*,
    shutdown::ShutdownGracefully,
    signalling::room_service::{
        CreateEndpointInRoom, CreateMemberInRoom, DeleteElements, Get,
        RoomService, RoomServiceError, StartRoom,
    },
    AppContext,
};

#[derive(Debug, Fail, Display)]
pub enum ControlApiError {
    /// Error while parsing local URI of element.
    LocalUri(LocalUriParseError),

    /// This error is rather abnormal, since what it catches must be caught at
    /// the level of the gRPC.
    TryFromProtobuf(TryFromProtobufError),

    /// This error is rather abnormal, since what it catches must be caught at
    /// the level of the gRPC.
    TryFromElement(TryFromElementError),

    /// [`MailboxError`] for [`RoomService`].
    #[display(fmt = "Room service mailbox error: {:?}", _0)]
    RoomServiceMailboxError(MailboxError),

    /// [`MailboxError`] which never can happen. This error needed
    /// for `fut_try!` macro because they use `From` trait.
    /// With this error we cover [`MailboxError`] in places where
    /// it cannot happen.
    ///
    /// __Never use this error.__
    #[display(fmt = "Mailbox error which never can happen. {:?}", _0)]
    UnknownMailboxErr(MailboxError),

    /// Wrapper aroung [`RoomServiceError`].
    RoomServiceError(RoomServiceError),
}

impl From<LocalUriParseError> for ControlApiError {
    fn from(from: LocalUriParseError) -> Self {
        ControlApiError::LocalUri(from)
    }
}

impl From<RoomServiceError> for ControlApiError {
    fn from(from: RoomServiceError) -> Self {
        Self::RoomServiceError(from)
    }
}

impl From<TryFromProtobufError> for ControlApiError {
    fn from(from: TryFromProtobufError) -> Self {
        ControlApiError::TryFromProtobuf(from)
    }
}

impl From<TryFromElementError> for ControlApiError {
    fn from(from: TryFromElementError) -> Self {
        ControlApiError::TryFromElement(from)
    }
}

/// Try to unwrap some [`Result`] and if it `Err` then return err future with
/// [`ControlApiError`].
///
/// __Note:__ this macro returns [`Either::B`].
macro_rules! fut_try {
    ($call:expr) => {
        match $call {
            Ok(o) => o,
            Err(e) => {
                return Either::B(futures::future::err(ControlApiError::from(
                    e,
                )))
            }
        }
    };
}

/// Macro for parse [`LocalUri`] and send error to client if some error occurs.
///
/// See `send_error_response` doc for details about arguments for this macro.
macro_rules! parse_local_uri {
    ($uri:expr, $ctx:expr, $sink:expr, $response:ty) => {
        match StatefulLocalUri::try_from($uri.as_ref()) {
            Ok(o) => o,
            Err(e) => {
                let error: ErrorResponse = e.into();
                send_error_response!($ctx, $sink, error, $response);
            }
        }
    };
}

/// Macro for send [`Error`] to client and `return` from current function.
///
/// `$error_code` - some object which can tranform into [`Error`] by `Into`
/// trait.
///
/// `$response` - is type of response ([`GetResponse`], [`Response`]
/// etc).
///
/// `$ctx` - context where `Future` for send gRPC response will be spawned.
///
/// `$sink` - `grpcio` sink for response.
macro_rules! send_error_response {
    ($ctx:tt, $sink:tt, $error_code:expr, $response:ty) => {
        let mut response = <$response>::new();
        let error: Error = $error_code.into();
        response.set_error(error);

        $ctx.spawn($sink.success(response).map_err(|e| {
            warn!("Error while sending Error response by gRPC. {:?}", e)
        }));

        return;
    };
}

/// Type alias for success [`CreateResponse`]'s sids.
type Sids = HashMap<String, String>;

#[derive(Clone)]
struct ControlApiService {
    /// [`Addr`] of [`RoomService`].
    room_service: Addr<RoomService>,

    /// Global app context.
    app: AppContext,
}

impl ControlApiService {
    fn get_sid(
        &self,
        room_id: &RoomId,
        member_id: &MemberId,
        credentials: &str,
    ) -> String {
        format!(
            "{}/{}/{}/{}",
            self.app.config.client.public_url, room_id, member_id, credentials
        )
    }

    /// Implementation of `Create` method for `Room` element.
    pub fn create_room(
        &mut self,
        req: &CreateRequest,
        uri: LocalUri<IsRoomId>,
    ) -> impl Future<Item = Sids, Error = ControlApiError> {
        let room = fut_try!(RoomSpec::try_from_protobuf(
            uri.room_id().clone(),
            req.get_room()
        ));

        let sid: HashMap<String, String> = fut_try!(room.members())
            .iter()
            .map(|(id, member)| {
                let uri =
                    self.get_sid(uri.room_id(), &id, member.credentials());

                (id.clone().to_string(), uri)
            })
            .collect();

        Either::A(
            self.room_service
                .send(StartRoom {
                    id: uri,
                    spec: room,
                })
                .map_err(ControlApiError::RoomServiceMailboxError)
                .and_then(move |r| {
                    r.map_err(ControlApiError::from).map(|_| sid)
                }),
        )
    }

    /// Implementation of `Create` method for `Member` element.
    pub fn create_member(
        &mut self,
        req: &CreateRequest,
        uri: LocalUri<IsMemberId>,
    ) -> impl Future<Item = Sids, Error = ControlApiError> {
        let spec = fut_try!(MemberSpec::try_from(req.get_member()));

        let sid =
            self.get_sid(uri.room_id(), uri.member_id(), spec.credentials());
        let mut sids = HashMap::new();
        sids.insert(uri.member_id().to_string(), sid);

        Either::A(
            self.room_service
                .send(CreateMemberInRoom { uri, spec })
                .map_err(ControlApiError::RoomServiceMailboxError)
                .and_then(|r| r.map_err(ControlApiError::from).map(|_| sids)),
        )
    }

    /// Implementation of `Create` method for `WebRtcPublishEndpoint` and
    /// `WebRtcPlayEndpoint` elements.
    pub fn create_endpoint(
        &mut self,
        req: &CreateRequest,
        uri: LocalUri<IsEndpointId>,
    ) -> impl Future<Item = Sids, Error = ControlApiError> {
        let endpoint = fut_try!(Endpoint::try_from(req));

        Either::A(
            self.room_service
                .send(CreateEndpointInRoom {
                    uri,
                    spec: endpoint,
                })
                .map_err(ControlApiError::RoomServiceMailboxError)
                .and_then(|r| {
                    r.map_err(ControlApiError::from).map(|_| HashMap::new())
                }),
        )
    }
}

impl ControlApi for ControlApiService {
    /// Implementation for `Create` method of gRPC control API.
    fn create(
        &mut self,
        ctx: RpcContext,
        req: CreateRequest,
        sink: UnarySink<CreateResponse>,
    ) {
        macro_rules! send_error_response_code {
            ($response_code:expr) => {
                send_error_response!(
                    ctx,
                    sink,
                    ErrorResponse::new($response_code, &req.get_id()),
                    CreateResponse
                )
            };
        }

        let response_fut: Box<
            dyn Future<Item = Sids, Error = ControlApiError> + Send,
        > = match parse_local_uri!(req.get_id(), ctx, sink, CreateResponse) {
            StatefulLocalUri::Room(local_uri) => {
                if req.has_room() {
                    Box::new(self.create_room(&req, local_uri))
                } else {
                    send_error_response_code!(
                        ErrorCode::ElementIdForRoomButElementIsNot
                    );
                }
            }
            StatefulLocalUri::Member(local_uri) => {
                if req.has_member() {
                    Box::new(self.create_member(&req, local_uri))
                } else {
                    send_error_response_code!(
                        ErrorCode::ElementIdForMemberButElementIsNot
                    );
                }
            }
            StatefulLocalUri::Endpoint(local_uri) => {
                if req.has_webrtc_pub() || req.has_webrtc_play() {
                    Box::new(self.create_endpoint(&req, local_uri))
                } else {
                    send_error_response_code!(
                        ErrorCode::ElementIdForEndpointButElementIsNot
                    );
                }
            }
        };
        ctx.spawn(response_fut.then(move |result| {
            let mut response = CreateResponse::new();
            match result {
                Ok(sid) => {
                    response.set_sid(sid);
                }
                Err(e) => response.set_error(ErrorResponse::from(e).into()),
            }
            sink.success(response).map_err(|e| {
                warn!("Error while sending Create response by gRPC. {:?}", e)
            })
        }));
    }

    /// Implementation for `Apply` method of gRPC control API.
    fn apply(
        &mut self,
        ctx: RpcContext,
        _req: ApplyRequest,
        sink: UnarySink<Response>,
    ) {
        ctx.spawn(
            sink.fail(RpcStatus::new(
                RpcStatusCode::Unimplemented,
                Some("Apply method currently is unimplemented.".to_string()),
            ))
            .map(|_| {
                info!(
                    "An unimplemented gRPC Control API method 'Apply' was \
                     called."
                );
            })
            .map_err(|e| {
                warn!("Unimplemented method Apply error: {:?}", e);
            }),
        );
    }

    /// Implementation for `Delete` method of gRPC control API.
    fn delete(
        &mut self,
        ctx: RpcContext,
        req: IdRequest,
        sink: UnarySink<Response>,
    ) {
        let mut delete_elements = DeleteElements::new();

        for id in req.get_id() {
            let uri: StatefulLocalUri =
                parse_local_uri!(id, ctx, sink, Response);
            delete_elements.add_uri(uri);
        }

        let delete_elements = match delete_elements.validate() {
            Ok(d) => d,
            Err(e) => {
                send_error_response!(
                    ctx,
                    sink,
                    ErrorResponse::from(e),
                    Response
                );
            }
        };

        ctx.spawn(
            self.room_service
                .send(delete_elements)
                .map_err(ControlApiError::RoomServiceMailboxError)
                .and_then(|r| r.map_err(ControlApiError::from))
                .then(move |result| {
                    let mut response = Response::new();
                    if let Err(e) = result {
                        response.set_error(ErrorResponse::from(e).into());
                    }
                    sink.success(response)
                })
                .map_err(|e| {
                    warn!(
                        "Error while sending response on Delete request by \
                         gRPC: {:?}",
                        e
                    )
                }),
        );
    }

    /// Implementation for `Get` method of gRPC control API.
    fn get(
        &mut self,
        ctx: RpcContext,
        req: IdRequest,
        sink: UnarySink<GetResponse>,
    ) {
        let mut uris = Vec::new();
        for id in req.get_id() {
            let local_uri = parse_local_uri!(id, ctx, sink, GetResponse);
            uris.push(local_uri);
        }
        ctx.spawn(
            self.room_service
                .send(Get(uris))
                .map_err(ControlApiError::RoomServiceMailboxError)
                .and_then(|r| r.map_err(ControlApiError::from))
                .then(|res| {
                    let mut response = GetResponse::new();
                    match res {
                        Ok(elements) => {
                            response.set_elements(
                                elements
                                    .into_iter()
                                    .map(|(id, value)| (id.to_string(), value))
                                    .collect(),
                            );
                        }
                        Err(e) => {
                            response.set_error(ErrorResponse::from(e).into());
                        }
                    }
                    sink.success(response)
                })
                .map_err(|e| {
                    warn!(
                        "Error while sending response on Get request by gRPC: \
                         {:?}",
                        e
                    )
                }),
        );
    }
}

/// Actor wrapper for `grcio` gRPC server.
#[allow(clippy::module_name_repetitions)]
pub struct GrpcServer {
    server: Server,
}

impl Actor for GrpcServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.server.start();
        info!("gRPC Control API server started.");
    }
}

impl Handler<ShutdownGracefully> for GrpcServer {
    type Result = ResponseFuture<(), ()>;

    fn handle(
        &mut self,
        _: ShutdownGracefully,
        _: &mut Self::Context,
    ) -> Self::Result {
        info!(
            "gRPC Control API server received ShutdownGracefully message so \
             shutting down.",
        );
        Box::new(self.server.shutdown().map_err(|e| {
            warn!(
                "Error while graceful shutdown of gRPC Contro API server: {:?}",
                e
            )
        }))
    }
}

/// Run gRPC server in actix actor.
pub fn run(room_repo: Addr<RoomService>, app: AppContext) -> Addr<GrpcServer> {
    let bind_ip = app.config.control.grpc.bind_ip.to_string();
    let bind_port = app.config.control.grpc.bind_port;
    let cq_count = app.config.control.grpc.completion_queue_count;

    let service = create_control_api(ControlApiService {
        app,
        room_service: room_repo,
    });
    let env = Arc::new(Environment::new(cq_count));

    info!("Starting gRPC server on {}:{}", bind_ip, bind_port);

    let server = ServerBuilder::new(env)
        .register_service(service)
        .bind(bind_ip, bind_port)
        .build()
        .unwrap();

    GrpcServer::start_in_arbiter(&Arbiter::new(), move |_| GrpcServer {
        server,
    })
}
