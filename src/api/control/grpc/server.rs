//! Implementation of [Control API] gRPC server.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{collections::HashMap, convert::TryFrom, sync::Arc};

use actix::{
    Actor, Addr, Arbiter, Context, Handler, MailboxError, ResponseFuture,
};
use derive_more::Display;
use failure::Fail;
use futures::future::{self, Either, Future, IntoFuture};
use grpcio::{
    Environment, RpcContext, RpcStatus, RpcStatusCode, Server, ServerBuilder,
    UnarySink,
};
use medea_control_api_proto::grpc::{
    control_api::{
        ApplyRequest, CreateRequest, CreateResponse, Element, GetResponse,
        IdRequest, Response,
    },
    control_api_grpc::{create_control_api, ControlApi},
};

use crate::{
    api::control::{
        error_codes::{ErrorCode, ErrorResponse},
        local_uri::{
            LocalUri, LocalUriParseError, StatefulLocalUri, ToEndpoint,
            ToMember,
        },
        EndpointSpec, MemberId, MemberSpec, RoomId, RoomSpec,
        TryFromElementError, TryFromProtobufError,
    },
    log::prelude::*,
    shutdown::ShutdownGracefully,
    signalling::room_service::{
        CreateEndpointInRoom, CreateMemberInRoom, CreateRoom, DeleteElements,
        Get, RoomService, RoomServiceError,
    },
    AppContext,
};

/// Errors which can happen while processing requests to gRPC [Control API].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Display, Fail)]
pub enum GrpcControlApiError {
    /// Error while parsing [`LocalUri`] of element.
    LocalUri(LocalUriParseError),

    /// Error which can happen while converting protobuf objects into interior
    /// [medea] [Control API] objects.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    /// [medea]: https://github.com/instrumentisto/medea
    TryFromProtobuf(TryFromProtobufError),

    /// This is __unexpected error__ because this kind of errors
    /// should be catched by `try_from_protobuf` function which returns
    /// [`TryFromProtobufError`].
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    TryFromElement(TryFromElementError),

    /// [`MailboxError`] for [`RoomService`].
    #[display(fmt = "Room service mailbox error: {:?}", _0)]
    RoomServiceMailboxError(MailboxError),

    /// Wrapper around [`RoomServiceError`].
    RoomServiceError(RoomServiceError),
}

impl From<LocalUriParseError> for GrpcControlApiError {
    fn from(from: LocalUriParseError) -> Self {
        Self::LocalUri(from)
    }
}

impl From<RoomServiceError> for GrpcControlApiError {
    fn from(from: RoomServiceError) -> Self {
        Self::RoomServiceError(from)
    }
}

impl From<TryFromProtobufError> for GrpcControlApiError {
    fn from(from: TryFromProtobufError) -> Self {
        Self::TryFromProtobuf(from)
    }
}

impl From<TryFromElementError> for GrpcControlApiError {
    fn from(from: TryFromElementError) -> Self {
        Self::TryFromElement(from)
    }
}

/// Type alias for success [`CreateResponse`]'s sids.
type Sids = HashMap<String, String>;

/// Service which provides gRPC [Control API] implementation.
#[derive(Clone)]
struct ControlApiService {
    /// [`Addr`] of [`RoomService`].
    room_service: Addr<RoomService>,

    /// Public URL of server. Address for exposed [Client API].
    ///
    /// [Client API]: https://tinyurl.com/yx9thsnr
    public_url: String,
}

impl ControlApiService {
    /// Returns [Control API] sid based on provided arguments and
    /// `MEDEA_CLIENT.PUBLIC_URL` config value.
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

    /// Implementation of `Create` method for [`Room`].
    fn create_room(
        &self,
        spec: RoomSpec,
    ) -> impl Future<Item = Sids, Error = GrpcControlApiError> {
        let sid = match spec.members() {
            Ok(members) => members
                .iter()
                .map(|(member_id, member)| {
                    let uri = self.get_sid(
                        spec.id(),
                        &member_id,
                        member.credentials(),
                    );
                    (member_id.clone().to_string(), uri)
                })
                .collect(),
            Err(e) => {
                return Either::B(future::err(
                    GrpcControlApiError::TryFromElement(e),
                ))
            }
        };

        Either::A(
            self.room_service
                .send(CreateRoom { spec })
                .map_err(GrpcControlApiError::RoomServiceMailboxError)
                .and_then(move |r| {
                    r.map_err(GrpcControlApiError::from).map(|_| sid)
                }),
        )
    }

    /// Implementation of `Create` method for [`Member`] element.
    fn create_member(
        &self,
        uri: LocalUri<ToMember>,
        spec: MemberSpec,
    ) -> impl Future<Item = Sids, Error = GrpcControlApiError> {
        let sid =
            self.get_sid(uri.room_id(), uri.member_id(), spec.credentials());
        let mut sids = HashMap::new();
        sids.insert(uri.member_id().to_string(), sid);

        self.room_service
            .send(CreateMemberInRoom { uri, spec })
            .map_err(GrpcControlApiError::RoomServiceMailboxError)
            .and_then(|r| r.map_err(GrpcControlApiError::from).map(|_| sids))
    }

    /// Implementation of `Create` method for [`Endpoint`] elements.
    fn create_endpoint(
        &self,
        uri: LocalUri<ToEndpoint>,
        spec: EndpointSpec,
    ) -> impl Future<Item = Sids, Error = GrpcControlApiError> {
        self.room_service
            .send(CreateEndpointInRoom { uri, spec })
            .map_err(GrpcControlApiError::RoomServiceMailboxError)
            .and_then(|r| {
                r.map_err(GrpcControlApiError::from).map(|_| HashMap::new())
            })
    }

    /// Creates element based on provided [`CreateRequest`].
    pub fn create_element(
        &self,
        mut req: CreateRequest,
    ) -> Box<dyn Future<Item = Sids, Error = ErrorResponse> + Send> {
        let uri = match StatefulLocalUri::try_from(req.take_id()) {
            Ok(uri) => uri,
            Err(e) => {
                return Box::new(future::err(e.into()));
            }
        };

        let elem = if let Some(elem) = req.el {
            elem
        } else {
            return Box::new(future::err(ErrorResponse::new(
                ErrorCode::NoElement,
                &uri,
            )));
        };

        match uri {
            StatefulLocalUri::Room(uri) => Box::new(
                RoomSpec::try_from((uri.take_room_id(), elem))
                    .map_err(ErrorResponse::from)
                    .map(|spec| {
                        self.create_room(spec).map_err(ErrorResponse::from)
                    })
                    .into_future()
                    .and_then(|create_result| create_result),
            ),
            StatefulLocalUri::Member(uri) => Box::new(
                MemberSpec::try_from((uri.member_id().clone(), elem))
                    .map_err(ErrorResponse::from)
                    .map(|spec| {
                        self.create_member(uri, spec)
                            .map_err(ErrorResponse::from)
                    })
                    .into_future()
                    .and_then(|create_result| create_result),
            ),
            StatefulLocalUri::Endpoint(uri) => Box::new(
                EndpointSpec::try_from((uri.endpoint_id().clone(), elem))
                    .map_err(ErrorResponse::from)
                    .map(|spec| {
                        self.create_endpoint(uri, spec)
                            .map_err(ErrorResponse::from)
                    })
                    .into_future()
                    .and_then(|create_result| create_result),
            ),
        }
    }

    /// Deletes element by [`IdRequest`].
    pub fn delete_element(
        &self,
        mut req: IdRequest,
    ) -> impl Future<Item = (), Error = ErrorResponse> {
        let mut delete_elements_msg = DeleteElements::new();
        for id in req.take_id().into_iter() {
            match StatefulLocalUri::try_from(id) {
                Ok(uri) => {
                    delete_elements_msg.add_uri(uri);
                }
                Err(e) => {
                    return future::Either::A(future::err(e.into()));
                }
            }
        }

        future::Either::B(
            delete_elements_msg
                .validate()
                .map_err(ErrorResponse::from)
                .map(|msg| self.room_service.send(msg))
                .into_future()
                .and_then(move |delete_result| {
                    delete_result.map_err(|err| {
                        ErrorResponse::from(
                            GrpcControlApiError::RoomServiceMailboxError(err),
                        )
                    })
                })
                .and_then(|result| result.map_err(ErrorResponse::from)),
        )
    }

    /// Returns requested by [`IdRequest`] [`Element`]s serialized to protobuf.
    pub fn get_element(
        &self,
        mut req: IdRequest,
    ) -> impl Future<Item = HashMap<String, Element>, Error = ErrorResponse>
    {
        let mut uris = Vec::new();
        for id in req.take_id().into_iter() {
            match StatefulLocalUri::try_from(id) {
                Ok(uri) => {
                    uris.push(uri);
                }
                Err(e) => {
                    return future::Either::A(future::err(e.into()));
                }
            }
        }

        future::Either::B(
            self.room_service
                .send(Get(uris))
                .map_err(GrpcControlApiError::RoomServiceMailboxError)
                .and_then(|r| r.map_err(GrpcControlApiError::from))
                .map(|elements: HashMap<StatefulLocalUri, Element>| {
                    elements
                        .into_iter()
                        .map(|(id, value)| (id.to_string(), value))
                        .collect()
                })
                .map_err(ErrorResponse::from),
        )
    }
}

impl ControlApi for ControlApiService {
    /// Implementation for `Create` method of gRPC [Control API].
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    fn create(
        &mut self,
        ctx: RpcContext,
        req: CreateRequest,
        sink: UnarySink<CreateResponse>,
    ) {
        ctx.spawn(
            self.create_element(req)
                .then(move |result| {
                    let mut response = CreateResponse::new();
                    match result {
                        Ok(sid) => {
                            response.set_sid(sid);
                        }
                        Err(e) => response.set_error(e.into()),
                    }
                    sink.success(response)
                })
                .map_err(|e| {
                    warn!(
                        "Error while sending Create response by gRPC. {:?}",
                        e
                    )
                }),
        );
    }

    /// Implementation for `Apply` method of gRPC [Control API] (__unimplemented
    /// atm__).
    ///
    /// Currently this is stub which returns fail response with
    /// [`RpcStatusCode::Unimplemented`].
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    fn apply(
        &mut self,
        ctx: RpcContext,
        _: ApplyRequest,
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

    /// Implementation for `Delete` method of gRPC [Control API].
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    fn delete(
        &mut self,
        ctx: RpcContext,
        req: IdRequest,
        sink: UnarySink<Response>,
    ) {
        ctx.spawn(
            self.delete_element(req)
                .then(move |result| {
                    let mut response = Response::new();
                    if let Err(e) = result {
                        response.set_error(e.into());
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

    /// Implementation for `Get` method of gRPC [Control API].
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    fn get(
        &mut self,
        ctx: RpcContext,
        req: IdRequest,
        sink: UnarySink<GetResponse>,
    ) {
        ctx.spawn(
            self.get_element(req)
                .then(|result| {
                    let mut response = GetResponse::new();
                    match result {
                        Ok(elements) => {
                            response.set_elements(
                                elements
                                    .into_iter()
                                    .map(|(id, value)| (id.to_string(), value))
                                    .collect(),
                            );
                        }
                        Err(e) => {
                            response.set_error(e.into());
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

/// Actor wrapper for [`grpcio`] gRPC server which provides dynamic [Control
/// API].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[allow(clippy::module_name_repetitions)]
pub struct GrpcServer(Server);

impl Actor for GrpcServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.0.start();
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
        Box::new(self.0.shutdown().map_err(|e| {
            warn!(
                "Error while graceful shutdown of gRPC Control API server: \
                 {:?}",
                e
            )
        }))
    }
}

/// Run gRPC [Control API] server in actix actor.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
pub fn run(room_repo: Addr<RoomService>, app: &AppContext) -> Addr<GrpcServer> {
    let bind_ip = app.config.server.control.grpc.bind_ip.to_string();
    let bind_port = app.config.server.control.grpc.bind_port;
    let cq_count = app.config.server.control.grpc.completion_queue_count;

    let service = create_control_api(ControlApiService {
        public_url: app.config.server.client.public_url.clone(),
        room_service: room_repo,
    });
    let env = Arc::new(Environment::new(cq_count));

    info!("Starting gRPC server on {}:{}", bind_ip, bind_port);

    let server = ServerBuilder::new(env)
        .register_service(service)
        .bind(bind_ip, bind_port)
        .build()
        .unwrap();

    GrpcServer::start_in_arbiter(&Arbiter::new(), move |_| GrpcServer(server))
}
