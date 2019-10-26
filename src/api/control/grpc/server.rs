//! Implementation of [Control API] gRPC server.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{
    collections::HashMap,
    convert::{From, TryFrom},
    sync::Arc,
};

use actix::{
    Actor, Addr, Arbiter, Context, Handler, MailboxError, ResponseFuture,
};
use derive_more::{Display, From};
use failure::Fail;
use futures::future::{self, Future, IntoFuture};
use grpcio::{Environment, RpcContext, Server, ServerBuilder, UnarySink};
use medea_control_api_proto::grpc::{
    api::{
        CreateRequest, CreateRequest_oneof_el as CreateRequestOneof,
        CreateResponse, Element, GetResponse, IdRequest, Response,
    },
    api_grpc::{create_control_api, ControlApi},
};

use crate::{
    api::control::{
        endpoints::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
        error_codes::{
            ErrorCode,
            ErrorCode::{ElementIdIsTooLong, ElementIdMismatch},
            ErrorResponse,
        },
        refs::{fid::ParseFidError, Fid, StatefulFid, ToMember, ToRoom},
        EndpointId, EndpointSpec, MemberId, MemberSpec, RoomSpec,
        TryFromProtobufError,
    },
    log::prelude::*,
    shutdown::ShutdownGracefully,
    signalling::room_service::{
        CreateEndpointInRoom, CreateMemberInRoom, CreateRoom, DeleteElements,
        Get, RoomService, RoomServiceError, Sids,
    },
    AppContext,
};

/// Errors which can happen while processing requests to gRPC [Control API].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
#[derive(Debug, Display, Fail, From)]
pub enum GrpcControlApiError {
    /// Error while parsing [`Fid`] of element.
    Fid(ParseFidError),

    /// Error which can happen while converting protobuf objects into interior
    /// [medea] [Control API] objects.
    ///
    /// [Control API]: https://tinyurl.com/yxsqplq7
    /// [medea]: https://github.com/instrumentisto/medea
    TryFromProtobuf(TryFromProtobufError),

    /// [`MailboxError`] for [`RoomService`].
    #[display(fmt = "Room service mailbox error: {:?}", _0)]
    RoomServiceMailboxError(MailboxError),

    /// Wrapper around [`RoomServiceError`].
    RoomServiceError(RoomServiceError),
}

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
    /// Implementation of `Create` method for [`Room`].
    fn create_room(
        &self,
        spec: RoomSpec,
    ) -> impl Future<Item = Sids, Error = GrpcControlApiError> {
        self.room_service
            .send(CreateRoom { spec })
            .map_err(GrpcControlApiError::RoomServiceMailboxError)
            .and_then(move |r| r.map_err(GrpcControlApiError::from))
    }

    /// Implementation of `Create` method for [`Member`] element.
    fn create_member(
        &self,
        id: MemberId,
        parent_fid: Fid<ToRoom>,
        spec: MemberSpec,
    ) -> impl Future<Item = Sids, Error = GrpcControlApiError> {
        self.room_service
            .send(CreateMemberInRoom {
                id,
                parent_fid,
                spec,
            })
            .map_err(GrpcControlApiError::RoomServiceMailboxError)
            .and_then(|r| r.map_err(GrpcControlApiError::from))
    }

    /// Implementation of `Create` method for [`Endpoint`] element.
    fn create_endpoint(
        &self,
        id: EndpointId,
        parent_fid: Fid<ToMember>,
        spec: EndpointSpec,
    ) -> impl Future<Item = Sids, Error = GrpcControlApiError> {
        self.room_service
            .send(CreateEndpointInRoom {
                id,
                parent_fid,
                spec,
            })
            .map_err(GrpcControlApiError::RoomServiceMailboxError)
            .and_then(|r| r.map_err(GrpcControlApiError::from))
    }

    /// Creates element based on provided [`CreateRequest`].
    pub fn create_element(
        &self,
        mut req: CreateRequest,
    ) -> Box<dyn Future<Item = Sids, Error = ErrorResponse> + Send> {
        let unparsed_parent_fid = req.take_parent_fid();
        let elem = if let Some(elem) = req.el {
            elem
        } else {
            return Box::new(future::err(ErrorResponse::new(
                ErrorCode::NoElement,
                &unparsed_parent_fid,
            )));
        };

        if unparsed_parent_fid.is_empty() {
            return Box::new(
                RoomSpec::try_from(elem)
                    .map_err(ErrorResponse::from)
                    .map(|spec| {
                        self.create_room(spec).map_err(ErrorResponse::from)
                    })
                    .into_future()
                    .and_then(|create_result| create_result),
            );
        }

        let parent_fid = match StatefulFid::try_from(unparsed_parent_fid) {
            Ok(parent_fid) => parent_fid,
            Err(e) => {
                return Box::new(future::err(e.into()));
            }
        };

        match parent_fid {
            StatefulFid::Room(parent_fid) => match elem {
                CreateRequestOneof::member(mut member) => {
                    let id: MemberId = member.take_id().into();
                    Box::new(
                        MemberSpec::try_from(member)
                            .map_err(ErrorResponse::from)
                            .map(|spec| {
                                self.create_member(id, parent_fid, spec)
                                    .map_err(ErrorResponse::from)
                            })
                            .into_future()
                            .and_then(|create_result| create_result),
                    )
                }
                _ => Box::new(future::err(ErrorResponse::new(
                    ElementIdMismatch,
                    &parent_fid,
                ))),
            },
            StatefulFid::Member(parent_fid) => {
                let (endpoint, id) = match elem {
                    CreateRequestOneof::webrtc_play(mut play) => (
                        WebRtcPlayEndpoint::try_from(&play)
                            .map(EndpointSpec::from),
                        play.take_id().into(),
                    ),
                    CreateRequestOneof::webrtc_pub(mut publish) => (
                        Ok(WebRtcPublishEndpoint::from(&publish))
                            .map(EndpointSpec::from),
                        publish.take_id().into(),
                    ),
                    _ => {
                        return Box::new(future::err(ErrorResponse::new(
                            ElementIdMismatch,
                            &parent_fid,
                        )))
                    }
                };
                Box::new(
                    endpoint
                        .map_err(ErrorResponse::from)
                        .map(move |spec| {
                            self.create_endpoint(id, parent_fid, spec)
                                .map_err(ErrorResponse::from)
                        })
                        .into_future()
                        .and_then(|create_res| create_res),
                )
            }
            StatefulFid::Endpoint(_) => Box::new(future::err(
                ErrorResponse::new(ElementIdIsTooLong, &parent_fid),
            )),
        }
    }

    /// Deletes element by [`IdRequest`].
    pub fn delete_element(
        &self,
        mut req: IdRequest,
    ) -> impl Future<Item = (), Error = ErrorResponse> {
        let mut delete_elements_msg = DeleteElements::new();
        for id in req.take_fid().into_iter() {
            match StatefulFid::try_from(id) {
                Ok(fid) => {
                    delete_elements_msg.add_fid(fid);
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
        let mut fids = Vec::new();
        for id in req.take_fid().into_iter() {
            match StatefulFid::try_from(id) {
                Ok(fid) => {
                    fids.push(fid);
                }
                Err(e) => {
                    return future::Either::A(future::err(e.into()));
                }
            }
        }

        future::Either::B(
            self.room_service
                .send(Get(fids))
                .map_err(GrpcControlApiError::RoomServiceMailboxError)
                .and_then(|r| r.map_err(GrpcControlApiError::from))
                .map(|elements: HashMap<StatefulFid, Element>| {
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
                        "Error while sending response on 'Delete' request by \
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
                        "Error while sending response on 'Get' request by \
                         gRPC: {:?}",
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
    let cq_count = 2;

    let service = create_control_api(ControlApiService {
        public_url: app.config.server.client.http.public_url.clone(),
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
