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
use futures::{
    compat::{Compat, Compat01As03, Future01CompatExt},
    future::{
        self, BoxFuture, Future, FutureExt, LocalBoxFuture, TryFutureExt,
    },
};
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
    utils::ResponseAnyFuture,
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
    ) -> BoxFuture<'static, Result<Sids, GrpcControlApiError>> {
        self.room_service
            .send(CreateRoom { spec })
            .map_err(GrpcControlApiError::RoomServiceMailboxError)
            .and_then(move |r| async { r.map_err(GrpcControlApiError::from) })
            .boxed()
    }

    /// Implementation of `Create` method for [`Member`] element.
    fn create_member(
        &self,
        id: MemberId,
        parent_fid: Fid<ToRoom>,
        spec: MemberSpec,
    ) -> BoxFuture<'static, Result<Sids, GrpcControlApiError>> {
        self.room_service
            .send(CreateMemberInRoom {
                id,
                parent_fid,
                spec,
            })
            .map_err(GrpcControlApiError::RoomServiceMailboxError)
            .and_then(|r| async { r.map_err(GrpcControlApiError::from) })
            .boxed()
    }

    /// Implementation of `Create` method for [`Endpoint`] element.
    fn create_endpoint(
        &self,
        id: EndpointId,
        parent_fid: Fid<ToMember>,
        spec: EndpointSpec,
    ) -> BoxFuture<'static, Result<Sids, GrpcControlApiError>> {
        self.room_service
            .send(CreateEndpointInRoom {
                id,
                parent_fid,
                spec,
            })
            .map_err(GrpcControlApiError::RoomServiceMailboxError)
            .and_then(|r| async { r.map_err(GrpcControlApiError::from) })
            .boxed()
    }

    /// Creates element based on provided [`CreateRequest`].
    pub fn create_element(
        &self,
        mut req: CreateRequest,
    ) -> BoxFuture<'static, Result<Sids, ErrorResponse>> {
        let unparsed_parent_fid = req.take_parent_fid();
        let elem = if let Some(elem) = req.el {
            elem
        } else {
            return async move {
                Err(ErrorResponse::new(
                    ErrorCode::NoElement,
                    &unparsed_parent_fid,
                ))
            }
            .boxed();
        };

        if unparsed_parent_fid.is_empty() {
            return match RoomSpec::try_from(elem).map_err(ErrorResponse::from) {
                Ok(spec) => {
                    self.create_room(spec).map_err(ErrorResponse::from).boxed()
                }
                Err(err) => async { Err(err) }.boxed(),
            };
        }

        let parent_fid = match StatefulFid::try_from(unparsed_parent_fid) {
            Ok(parent_fid) => parent_fid,
            Err(e) => {
                return async { Err(e.into()) }.boxed();
            }
        };

        match parent_fid {
            StatefulFid::Room(parent_fid) => match elem {
                CreateRequestOneof::member(mut member) => {
                    let id: MemberId = member.take_id().into();
                    match MemberSpec::try_from(member)
                        .map_err(ErrorResponse::from)
                    {
                        Ok(spec) => self
                            .create_member(id, parent_fid, spec)
                            .map_err(ErrorResponse::from)
                            .boxed(),
                        Err(err) => async { Err(err) }.boxed(),
                    }
                }
                _ => async move {
                    Err(ErrorResponse::new(ElementIdMismatch, &parent_fid))
                }
                .boxed(),
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
                        return async move {
                            Err(ErrorResponse::new(
                                ElementIdMismatch,
                                &parent_fid,
                            ))
                        }
                        .boxed()
                    }
                };

                match endpoint.map_err(ErrorResponse::from) {
                    Ok(spec) => self
                        .create_endpoint(id, parent_fid, spec)
                        .map_err(ErrorResponse::from)
                        .boxed(),
                    Err(err) => async move { Err(err) }.boxed(),
                }
            }
            StatefulFid::Endpoint(_) => async move {
                Err(ErrorResponse::new(ElementIdIsTooLong, &parent_fid))
            }
            .boxed(),
        }
    }

    /// Deletes element by [`IdRequest`].
    pub fn delete_element(
        &self,
        mut req: IdRequest,
    ) -> BoxFuture<'static, Result<(), ErrorResponse>> {
        let room_service = Clone::clone(&self.room_service);
        async move {
            let mut delete_elements_msg = DeleteElements::new();
            for id in req.take_fid().into_iter() {
                let fid = StatefulFid::try_from(id)?;
                delete_elements_msg.add_fid(fid);
            }
            room_service
                .send(delete_elements_msg.validate()?)
                .await
                .map_err(|err| {
                    ErrorResponse::from(
                        GrpcControlApiError::RoomServiceMailboxError(err),
                    )
                })??;
            Ok(())
        }
        .boxed()
    }

    /// Returns requested by [`IdRequest`] [`Element`]s serialized to protobuf.
    pub fn get_element(
        &self,
        mut req: IdRequest,
    ) -> BoxFuture<'static, Result<HashMap<String, Element>, ErrorResponse>>
    {
        let room_service = Clone::clone(&self.room_service);
        async move {
            let mut fids = Vec::new();
            for id in req.take_fid().into_iter() {
                let fid = StatefulFid::try_from(id)?;
                fids.push(fid);
            }

            let elements =
                room_service.send(Get(fids)).await.map_err(|err| {
                    ErrorResponse::from(
                        GrpcControlApiError::RoomServiceMailboxError(err),
                    )
                })??;

            let result = elements
                .into_iter()
                .map(|(id, value)| (id.to_string(), value))
                .collect();
            Ok(result)
        }
        .boxed()
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
        let create_element = self.create_element(req);
        ctx.spawn(
            async {
                let mut response = CreateResponse::new();
                match create_element.await {
                    Ok(sid) => {
                        response.set_sid(sid);
                    }
                    Err(e) => response.set_error(e.into()),
                }
                if let Err(err) = sink.success(response).compat().await {
                    warn!(
                        "Error while sending Create response by gRPC. {:?}",
                        err
                    )
                }
                Ok(())
            }
            .boxed()
            .compat(),
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
        let delete_element = self.delete_element(req);
        ctx.spawn(
            async {
                let mut response = Response::new();
                if let Err(e) = delete_element.await {
                    response.set_error(e.into());
                }
                if let Err(err) = sink.success(response).compat().await {
                    warn!(
                        "Error while sending response on 'Delete' request by \
                         gRPC: {:?}",
                        err
                    )
                }
                Ok(())
            }
            .boxed()
            .compat(),
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
        let get_element = self.get_element(req);
        ctx.spawn(
            async {
                let mut response = GetResponse::new();
                match get_element.await {
                    Ok(elements) => {
                        response.set_elements(elements);
                    }
                    Err(e) => {
                        response.set_error(e.into());
                    }
                }
                if let Err(err) = sink.success(response).compat().await {
                    warn!(
                        "Error while sending response on 'Get' request by \
                         gRPC: {:?}",
                        err
                    )
                }
                Ok(())
            }
            .boxed()
            .compat(),
        );
    }
}

/// Actor wrapper for [`grpcio`] gRPC server which provides dynamic [Control
/// API].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
pub struct GrpcServer(Server);

impl Actor for GrpcServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.0.start();
        info!("gRPC Control API server started.");
    }
}

impl Handler<ShutdownGracefully> for GrpcServer {
    type Result = ResponseAnyFuture<()>;

    fn handle(
        &mut self,
        _: ShutdownGracefully,
        _: &mut Self::Context,
    ) -> Self::Result {
        info!(
            "gRPC Control API server received ShutdownGracefully message so \
             shutting down.",
        );
        ResponseAnyFuture(
            Compat01As03::new(self.0.shutdown())
                .map_err(|e| {
                    warn!(
                        "Error while graceful shutdown of gRPC Control API \
                         server: {:?}",
                        e
                    )
                })
                .into_future()
                .map(|_| ())
                .boxed(),
        )
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
