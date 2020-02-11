//! Implementation of [Control API] gRPC server.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{
    collections::HashMap,
    convert::{From, TryFrom},
};

use actix::{Actor, Addr, Arbiter, Context, Handler, MailboxError};
use async_trait::async_trait;
use derive_more::{Display, From};
use failure::Fail;
use futures::future::{self, BoxFuture, FutureExt as _, TryFutureExt as _};
use medea_control_api_proto::grpc::{
    api as proto,
    api::control_api_server::{
        ControlApi, ControlApiServer as TonicControlApiServer,
    },
};
use tonic::{transport::Server, Status};

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
    ) -> BoxFuture<'static, Result<Sids, GrpcControlApiError>> {
        let send_result = self.room_service.send(CreateRoom { spec });
        async {
            Ok(send_result
                .await
                .map_err(GrpcControlApiError::RoomServiceMailboxError)??)
        }
        .boxed()
    }

    /// Implementation of `Create` method for [`Member`] element.
    fn create_member(
        &self,
        id: MemberId,
        parent_fid: Fid<ToRoom>,
        spec: MemberSpec,
    ) -> BoxFuture<'static, Result<Sids, GrpcControlApiError>> {
        let send_result = self.room_service.send(CreateMemberInRoom {
            id,
            parent_fid,
            spec,
        });
        async {
            Ok(send_result
                .await
                .map_err(GrpcControlApiError::RoomServiceMailboxError)??)
        }
        .boxed()
    }

    /// Implementation of `Create` method for [`Endpoint`] element.
    fn create_endpoint(
        &self,
        id: EndpointId,
        parent_fid: Fid<ToMember>,
        spec: EndpointSpec,
    ) -> BoxFuture<'static, Result<Sids, GrpcControlApiError>> {
        let send_result = self.room_service.send(CreateEndpointInRoom {
            id,
            parent_fid,
            spec,
        });
        async {
            Ok(send_result
                .await
                .map_err(GrpcControlApiError::RoomServiceMailboxError)??)
        }
        .boxed()
    }

    /// Creates element based on provided [`proto::CreateRequest`].
    pub fn create_element(
        &self,
        req: proto::CreateRequest,
    ) -> BoxFuture<'static, Result<Sids, ErrorResponse>> {
        let unparsed_parent_fid = req.parent_fid;
        let elem = if let Some(elem) = req.el {
            elem
        } else {
            return future::err(ErrorResponse::new(
                ErrorCode::NoElement,
                &unparsed_parent_fid,
            ))
            .boxed();
        };

        if unparsed_parent_fid.is_empty() {
            return match RoomSpec::try_from(elem).map_err(ErrorResponse::from) {
                Ok(spec) => self.create_room(spec).err_into().boxed(),
                Err(e) => future::err(e).boxed(),
            };
        }

        let parent_fid = match StatefulFid::try_from(unparsed_parent_fid) {
            Ok(parent_fid) => parent_fid,
            Err(e) => {
                return future::err(e.into()).boxed();
            }
        };

        match parent_fid {
            StatefulFid::Room(parent_fid) => match elem {
                proto::create_request::El::Member(member) => {
                    let id: MemberId = member.id.clone().into();
                    match MemberSpec::try_from(member)
                        .map_err(ErrorResponse::from)
                    {
                        Ok(spec) => self
                            .create_member(id, parent_fid, spec)
                            .err_into()
                            .boxed(),
                        Err(e) => future::err(e).boxed(),
                    }
                }
                _ => future::err(ErrorResponse::new(
                    ElementIdMismatch,
                    &parent_fid,
                ))
                .boxed(),
            },
            StatefulFid::Member(parent_fid) => {
                let (endpoint, id) = match elem {
                    proto::create_request::El::WebrtcPlay(play) => (
                        WebRtcPlayEndpoint::try_from(&play)
                            .map(EndpointSpec::from),
                        play.id.into(),
                    ),
                    proto::create_request::El::WebrtcPub(publish) => (
                        WebRtcPublishEndpoint::try_from(&publish)
                            .map(EndpointSpec::from),
                        publish.id.into(),
                    ),
                    _ => {
                        return future::err(ErrorResponse::new(
                            ElementIdMismatch,
                            &parent_fid,
                        ))
                        .boxed()
                    }
                };

                match endpoint.map_err(ErrorResponse::from) {
                    Ok(spec) => self
                        .create_endpoint(id, parent_fid, spec)
                        .err_into()
                        .boxed(),
                    Err(e) => future::err(e).boxed(),
                }
            }
            StatefulFid::Endpoint(_) => {
                future::err(ErrorResponse::new(ElementIdIsTooLong, &parent_fid))
                    .boxed()
            }
        }
    }

    /// Deletes element by [`proto::IdRequest`].
    pub fn delete_element(
        &self,
        req: proto::IdRequest,
    ) -> BoxFuture<'static, Result<(), ErrorResponse>> {
        let room_service = self.room_service.clone();
        async move {
            let mut delete_elements_msg = DeleteElements::new();
            for id in req.fid {
                let fid = StatefulFid::try_from(id)?;
                delete_elements_msg.add_fid(fid);
            }
            room_service
                .send(delete_elements_msg.validate()?)
                .await
                .map_err(|e| {
                    ErrorResponse::from(
                        GrpcControlApiError::RoomServiceMailboxError(e),
                    )
                })??;
            Ok(())
        }
        .boxed()
    }

    /// Returns requested by [`proto::IdRequest`] [`proto::Element`]s serialized
    /// to protobuf.
    pub fn get_element(
        &self,
        req: proto::IdRequest,
    ) -> BoxFuture<
        'static,
        Result<HashMap<String, proto::Element>, ErrorResponse>,
    > {
        let room_service = self.room_service.clone();
        async move {
            let mut fids = Vec::new();
            for id in req.fid {
                let fid = StatefulFid::try_from(id)?;
                fids.push(fid);
            }

            let elements =
                room_service.send(Get(fids)).await.map_err(|err| {
                    ErrorResponse::from(
                        GrpcControlApiError::RoomServiceMailboxError(err),
                    )
                })??;

            Ok(elements
                .into_iter()
                .map(|(id, value)| (id.to_string(), value))
                .collect())
        }
        .boxed()
    }
}

#[async_trait]
impl ControlApi for ControlApiService {
    async fn create(
        &self,
        request: tonic::Request<proto::CreateRequest>,
    ) -> Result<tonic::Response<proto::CreateResponse>, Status> {
        debug!("Create Request: {:?}", request);
        let create_response =
            match self.create_element(request.into_inner()).await {
                Ok(sid) => proto::CreateResponse { sid, error: None },
                Err(err) => proto::CreateResponse {
                    sid: HashMap::new(),
                    error: Some(err.into()),
                },
            };

        Ok(tonic::Response::new(create_response))
    }

    async fn delete(
        &self,
        request: tonic::Request<proto::IdRequest>,
    ) -> Result<tonic::Response<proto::Response>, Status> {
        let response = match self.delete_element(request.into_inner()).await {
            Ok(_) => proto::Response { error: None },
            Err(e) => proto::Response {
                error: Some(e.into()),
            },
        };

        Ok(tonic::Response::new(response))
    }

    async fn get(
        &self,
        request: tonic::Request<proto::IdRequest>,
    ) -> Result<tonic::Response<proto::GetResponse>, Status> {
        let response = match self.get_element(request.into_inner()).await {
            Ok(elements) => proto::GetResponse {
                elements,
                error: None,
            },
            Err(e) => proto::GetResponse {
                elements: HashMap::new(),
                error: Some(e.into()),
            },
        };

        Ok(tonic::Response::new(response))
    }
}

/// Actor wrapper for [`tonic`] gRPC server which provides dynamic [Control
/// API].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
pub struct GrpcServer(Option<futures::channel::oneshot::Sender<()>>);

impl Actor for GrpcServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        info!("gRPC Control API server started.");
    }
}

impl Handler<ShutdownGracefully> for GrpcServer {
    type Result = ();

    fn handle(
        &mut self,
        _: ShutdownGracefully,
        _: &mut Self::Context,
    ) -> Self::Result {
        info!(
            "gRPC Control API server received ShutdownGracefully message so \
             shutting down.",
        );
        if let Some(grpc_shutdown) = self.0.take() {
            grpc_shutdown.send(()).ok();
        }
    }
}

/// Run gRPC [Control API] server in actix actor.
///
/// [Control API]: https://tinyurl.com/yxsqplq7
pub async fn run(
    room_repo: Addr<RoomService>,
    app: &AppContext,
) -> Addr<GrpcServer> {
    let bind_ip = app.config.server.control.grpc.bind_ip.to_string();
    let bind_port = app.config.server.control.grpc.bind_port;

    let service = TonicControlApiServer::new(ControlApiService {
        public_url: app.config.server.client.http.public_url.clone(),
        room_service: room_repo,
    });

    info!("Starting gRPC server on {}:{}", bind_ip, bind_port);

    let (grpc_shutdown_tx, grpc_shutdown_rx) =
        futures::channel::oneshot::channel();

    let addr = format!("{}:{}", bind_ip, bind_port).parse().unwrap();
    Arbiter::spawn(async move {
        Server::builder()
            .add_service(service)
            .serve_with_shutdown(addr, async move {
                grpc_shutdown_rx.await.ok();
            })
            .await
            .unwrap();
    });

    GrpcServer::start_in_arbiter(&Arbiter::new(), move |_| {
        GrpcServer(Some(grpc_shutdown_tx))
    })
}
