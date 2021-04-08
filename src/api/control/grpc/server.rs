//! Implementation of [Control API] gRPC server.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{
    collections::HashMap,
    convert::{From, TryFrom},
    net::SocketAddr,
};

use actix::{Actor, Addr, Arbiter, Context, Handler, MailboxError, System};
use async_trait::async_trait;
use derive_more::{Display, From};
use failure::Fail;
use futures::channel::oneshot;
use medea_client_api_proto::MemberId;
use medea_control_api_proto::grpc::{
    api as proto,
    api::control_api_server::{
        ControlApi, ControlApiServer as TonicControlApiServer,
    },
};
use tonic::{
    transport::{self, Server},
    Status,
};

use crate::{
    api::control::{
        endpoints::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
        error_codes::{
            ErrorCode,
            ErrorCode::{
                ElementIdIsTooLong, ElementIdMismatch, UnimplementedCall,
            },
            ErrorResponse,
        },
        refs::{fid::ParseFidError, Fid, StatefulFid, ToMember, ToRoom},
        EndpointId, EndpointSpec, MemberSpec, RoomSpec, TryFromProtobufError,
    },
    log::prelude::*,
    shutdown::ShutdownGracefully,
    signalling::room_service::{
        ApplyMember, ApplyRoom, CreateEndpointInRoom, CreateMemberInRoom,
        CreateRoom, DeleteElements, Get, RoomService, RoomServiceError, Sids,
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
struct ControlApiService(Addr<RoomService>);

impl ControlApiService {
    /// Implementation of `Create` method for [`Room`].
    ///
    /// [`Room`]: crate::signalling::room::Room
    async fn create_room(
        &self,
        spec: RoomSpec,
    ) -> Result<Sids, GrpcControlApiError> {
        Ok(self.0.send(CreateRoom { spec }).await??)
    }

    /// Implementation of `Create` method for [`Member`] element.
    ///
    /// [`Member`]: crate::signalling::elements::Member
    async fn create_member(
        &self,
        id: MemberId,
        parent_fid: Fid<ToRoom>,
        spec: MemberSpec,
    ) -> Result<Sids, GrpcControlApiError> {
        Ok(self
            .0
            .send(CreateMemberInRoom {
                id,
                parent_fid,
                spec,
            })
            .await??)
    }

    /// Implementation of `Create` method for `Endpoint` element.
    async fn create_endpoint(
        &self,
        id: EndpointId,
        parent_fid: Fid<ToMember>,
        spec: EndpointSpec,
    ) -> Result<Sids, GrpcControlApiError> {
        Ok(self
            .0
            .send(CreateEndpointInRoom {
                id,
                parent_fid,
                spec,
            })
            .await??)
    }

    /// Parses the provided [`proto::ApplyRequest`] and sends [`ApplyRoom`] or
    /// [`ApplyMember`] message to [`RoomService`].
    async fn apply_element(
        &self,
        req: proto::ApplyRequest,
    ) -> Result<Sids, ErrorResponse> {
        let unparsed_fid = req.parent_fid;
        let elem = if let Some(elem) = req.el {
            elem
        } else {
            return Err(ErrorResponse::new(
                ErrorCode::NoElement,
                &unparsed_fid,
            ));
        };

        let parent_fid = StatefulFid::try_from(unparsed_fid)?;
        match parent_fid {
            StatefulFid::Room(fid) => match elem {
                proto::apply_request::El::Room(_) => Ok(self
                    .0
                    .send(ApplyRoom {
                        id: fid.take_room_id(),
                        spec: RoomSpec::try_from(elem)?,
                    })
                    .await
                    .map_err(GrpcControlApiError::from)??),
                _ => Err(ErrorResponse::new(ElementIdMismatch, &fid)),
            },
            StatefulFid::Member(fid) => match elem {
                proto::apply_request::El::Member(member) => Ok(self
                    .0
                    .send(ApplyMember {
                        fid,
                        spec: MemberSpec::try_from(member)?,
                    })
                    .await
                    .map(|_| Sids::new())
                    .map_err(GrpcControlApiError::from)?),
                _ => Err(ErrorResponse::new(ElementIdMismatch, &fid)),
            },
            StatefulFid::Endpoint(_) => Err(ErrorResponse::with_explanation(
                UnimplementedCall,
                String::from(
                    "Apply method for Endpoints is not currently supported.",
                ),
                None,
            )),
        }
    }

    /// Creates element based on provided [`proto::CreateRequest`].
    async fn create_element(
        &self,
        req: proto::CreateRequest,
    ) -> Result<Sids, ErrorResponse> {
        let unparsed_parent_fid = req.parent_fid;
        let elem = if let Some(elem) = req.el {
            elem
        } else {
            return Err(ErrorResponse::new(
                ErrorCode::NoElement,
                &unparsed_parent_fid,
            ));
        };

        if unparsed_parent_fid.is_empty() {
            return Ok(self.create_room(RoomSpec::try_from(elem)?).await?);
        }

        let parent_fid = StatefulFid::try_from(unparsed_parent_fid)?;
        match parent_fid {
            StatefulFid::Room(parent_fid) => match elem {
                proto::create_request::El::Member(member) => {
                    let id: MemberId = member.id.clone().into();
                    let member_spec = MemberSpec::try_from(member)?;
                    Ok(self.create_member(id, parent_fid, member_spec).await?)
                }
                _ => Err(ErrorResponse::new(ElementIdMismatch, &parent_fid)),
            },
            StatefulFid::Member(parent_fid) => {
                let (endpoint_spec, id) = match elem {
                    proto::create_request::El::WebrtcPlay(play) => (
                        EndpointSpec::from(WebRtcPlayEndpoint::try_from(
                            &play,
                        )?),
                        play.id.into(),
                    ),
                    proto::create_request::El::WebrtcPub(publish) => (
                        EndpointSpec::from(WebRtcPublishEndpoint::from(
                            &publish,
                        )),
                        publish.id.into(),
                    ),
                    _ => {
                        return Err(ErrorResponse::new(
                            ElementIdMismatch,
                            &parent_fid,
                        ))
                    }
                };

                Ok(self.create_endpoint(id, parent_fid, endpoint_spec).await?)
            }
            StatefulFid::Endpoint(_) => {
                Err(ErrorResponse::new(ElementIdIsTooLong, &parent_fid))
            }
        }
    }

    /// Deletes element by [`proto::IdRequest`].
    async fn delete_element(
        &self,
        req: proto::IdRequest,
    ) -> Result<(), GrpcControlApiError> {
        let mut delete_elements_msg = DeleteElements::new();
        for id in req.fid {
            let fid = StatefulFid::try_from(id)?;
            delete_elements_msg.add_fid(fid);
        }
        self.0.send(delete_elements_msg.validate()?).await??;
        Ok(())
    }

    /// Returns requested by [`proto::IdRequest`] [`proto::Element`]s serialized
    /// to protobuf.
    async fn get_element(
        &self,
        req: proto::IdRequest,
    ) -> Result<HashMap<String, proto::Element>, GrpcControlApiError> {
        let mut fids = Vec::new();
        for id in req.fid {
            let fid = StatefulFid::try_from(id)?;
            fids.push(fid);
        }

        let elements = self.0.send(Get(fids)).await??;

        Ok(elements
            .into_iter()
            .map(|(id, value)| (id.to_string(), value))
            .collect())
    }
}

/// Converts [`Sids`] to a [`HashMap`] of [`String`]s for gRPC Control API
/// protocol.
fn proto_sids(sids: Sids) -> HashMap<String, String> {
    sids.into_iter()
        .map(|(id, sid)| (id.to_string(), sid.to_string()))
        .collect()
}

#[async_trait]
impl ControlApi for ControlApiService {
    /// Creates a new [`Element`] with a given ID.
    ///
    /// Not idempotent. Errors if an [`Element`] with the same ID already
    /// exists.
    ///
    /// Propagates request to [`ControlApiService::create_element`].
    ///
    /// [`Element`]: proto::create_request::El
    async fn create(
        &self,
        request: tonic::Request<proto::CreateRequest>,
    ) -> Result<tonic::Response<proto::CreateResponse>, Status> {
        debug!("Create gRPC Request: [{:?}]", request);
        let create_response =
            match self.create_element(request.into_inner()).await {
                Ok(sid) => proto::CreateResponse {
                    sid: proto_sids(sid),
                    error: None,
                },
                Err(e) => proto::CreateResponse {
                    sid: HashMap::new(),
                    error: Some(e.into()),
                },
            };
        Ok(tonic::Response::new(create_response))
    }

    /// Removes an [`Element`] by its ID.
    ///
    /// Allows referring multiple [`Element`]s on the last two levels.
    /// Idempotent. If no [`Element`]s with such IDs exist, then succeeds.
    ///
    /// Propagates request to [`ControlApiService::delete_element`].
    ///
    /// [`Element`]: proto::Element
    async fn delete(
        &self,
        request: tonic::Request<proto::IdRequest>,
    ) -> Result<tonic::Response<proto::Response>, Status> {
        debug!("Delete gRPC Request: [{:?}]", request);
        let response = match self.delete_element(request.into_inner()).await {
            Ok(_) => proto::Response { error: None },
            Err(e) => proto::Response {
                error: Some(ErrorResponse::from(e).into()),
            },
        };
        Ok(tonic::Response::new(response))
    }

    /// Returns an [`Element`] by its ID.
    ///
    /// Allows referring multiple [`Element`]s.
    /// If no ID specified, returns all the declared [`Element`]s.
    ///
    /// Propagates request to [`ControlApiService::get_element`].
    ///
    /// [`Element`]: proto::Element
    async fn get(
        &self,
        request: tonic::Request<proto::IdRequest>,
    ) -> Result<tonic::Response<proto::GetResponse>, Status> {
        debug!("Get gRPC Request: [{:?}]", request);
        let response = match self.get_element(request.into_inner()).await {
            Ok(elements) => proto::GetResponse {
                elements,
                error: None,
            },
            Err(e) => proto::GetResponse {
                elements: HashMap::new(),
                error: Some(ErrorResponse::from(e).into()),
            },
        };
        Ok(tonic::Response::new(response))
    }

    /// Applies the given spec to an [`Element`] by its ID.
    ///
    /// Idempotent. If no [`Element`] with such ID exists, then it will be
    /// created, otherwise it will be reconfigured. [`Element`]s that exist, but
    /// are not specified in the provided spec will be removed.
    ///
    /// Propagates request to [`ControlApiService::apply_element`].
    ///
    /// [`Element`]: proto::apply_request::El
    async fn apply(
        &self,
        request: tonic::Request<proto::ApplyRequest>,
    ) -> Result<tonic::Response<proto::CreateResponse>, Status> {
        debug!("Apply gRPC Request: [{:?}]", request);
        let response = match self.apply_element(request.into_inner()).await {
            Ok(sid) => proto::CreateResponse {
                sid: proto_sids(sid),
                error: None,
            },
            Err(e) => proto::CreateResponse {
                sid: HashMap::new(),
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
            let _ = grpc_shutdown.send(());
        }
    }
}

/// Run gRPC [Control API] server in actix actor. Returns [`Addr`] of
/// [`GrpcServer`] [`Actor`] and [`oneshot::Receiver`] for [`transport::Error`]
/// that may fire when initializing [`GrpcServer`].
///
/// [Control API]: https://tinyurl.com/yxsqplq7
pub fn run(
    room_service: Addr<RoomService>,
    app: &AppContext,
) -> (
    Addr<GrpcServer>,
    oneshot::Receiver<Result<(), transport::Error>>,
) {
    let bind_ip = app.config.server.control.grpc.bind_ip;
    let bind_port = app.config.server.control.grpc.bind_port;

    info!("Starting gRPC server on {}:{}", bind_ip, bind_port);

    let bind_addr = SocketAddr::from((bind_ip, bind_port));
    let (grpc_shutdown_tx, grpc_shutdown_rx) = oneshot::channel();
    let (tonic_server_tx, tonic_server_rx) = oneshot::channel();
    let grpc_actor_addr =
        GrpcServer::start_in_arbiter(&Arbiter::new(), move |_| {
            Arbiter::spawn(async move {
                let result = Server::builder()
                    .add_service(TonicControlApiServer::new(ControlApiService(
                        room_service,
                    )))
                    .serve_with_shutdown(bind_addr, async move {
                        let _ = grpc_shutdown_rx.await;
                    })
                    .await;

                if let Err(err) = tonic_server_tx.send(result) {
                    error!(
                        "gRPC server failed to start, and error could not \
                        be propagated. Error details: {:?}",
                        err
                    );
                    System::current().stop();
                };
            });

            GrpcServer(Some(grpc_shutdown_tx))
        });

    (grpc_actor_addr, tonic_server_rx)
}
