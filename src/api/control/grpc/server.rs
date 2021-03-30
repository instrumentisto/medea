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
use medea_client_api_proto::MemberId;
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
        EndpointId, EndpointSpec, MemberSpec, RoomSpec, TryFromProtobufError,
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

#[async_trait]
impl ControlApi for ControlApiService {
    async fn create(
        &self,
        request: tonic::Request<proto::CreateRequest>,
    ) -> Result<tonic::Response<proto::CreateResponse>, Status> {
        debug!("Create gRPC Request: [{:?}]", request);
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
        debug!("Delete gRPC Request: [{:?}]", request);
        let response = match self.delete_element(request.into_inner()).await {
            Ok(_) => proto::Response { error: None },
            Err(e) => proto::Response {
                error: Some(ErrorResponse::from(e).into()),
            },
        };
        Ok(tonic::Response::new(response))
    }

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
#[allow(clippy::missing_panics_doc)]
pub async fn run(
    room_service: Addr<RoomService>,
    app: &AppContext,
) -> Addr<GrpcServer> {
    let bind_ip = app.config.server.control.grpc.bind_ip.to_string();
    let bind_port = app.config.server.control.grpc.bind_port;

    info!("Starting gRPC server on {}:{}", bind_ip, bind_port);

    let (grpc_shutdown_tx, grpc_shutdown_rx) =
        futures::channel::oneshot::channel();

    let addr = format!("{}:{}", bind_ip, bind_port).parse().unwrap();
    Arbiter::spawn(async move {
        Server::builder()
            .add_service(TonicControlApiServer::new(ControlApiService(
                room_service,
            )))
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
