//! Implementation of gRPC client for sending [`CallbackRequest`]s.

use std::{fmt, sync::Arc};

use futures::future::{BoxFuture, FutureExt as _, LocalBoxFuture};
#[rustfmt::skip]
use medea_control_api_proto::grpc::medea_callback::{
    callback_client::CallbackClient as ProtoCallbackClient
};
use futures::{Future, TryFutureExt};

use crate::api::control::callback::{
    clients::{CallbackClient, CallbackClientError},
    url::GrpcCallbackUrl,
    CallbackRequest,
};
use actix::{Actor, ActorFuture, Addr, Context, Handler, WrapFuture};
use actix_web::dev::Service;
use failure::_core::pin::Pin;
use parking_lot::Mutex;
use std::{cell::RefCell, rc::Rc};
use tonic::transport::Channel;

/// gRPC client for sending [`CallbackRequest`]s.
pub struct GrpcCallbackClient {
    /// [`grpcio`] gRPC client of Control API Callback service.
    client: Rc<RefCell<ProtoCallbackClient<Channel>>>,
}

pub type ActFuture<O> =
    Box<dyn ActorFuture<Actor = GrpcCallbackClient, Output = O>>;

impl Actor for GrpcCallbackClient {
    type Context = Context<Self>;
}

impl CallbackClient for Addr<GrpcCallbackClient> {
    fn send(
        &self,
        request: CallbackRequest,
    ) -> LocalBoxFuture<'static, Result<(), CallbackClientError>> {
        let this = self.clone();
        Box::pin(async move { Ok(this.send(request).await??) })
    }
}

impl Handler<CallbackRequest> for GrpcCallbackClient {
    type Result = ActFuture<Result<(), CallbackClientError>>;

    fn handle(
        &mut self,
        msg: CallbackRequest,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let client = Rc::clone(&self.client);
        Box::new(
            async move {
                client
                    .borrow_mut()
                    .on_event(tonic::Request::new(msg.into()))
                    .await?;

                Ok(())
            }
            .into_actor(self),
        )
    }
}

impl fmt::Debug for GrpcCallbackClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("GrpcCallbackClient")
            .field("client", &"/* Cannot be printed */")
            .finish()
    }
}

impl GrpcCallbackClient {
    /// Returns gRPC client for provided [`GrpcCallbackUrl`].
    ///
    /// Note that this function doesn't check availability of gRPC server on
    /// provided [`GrpcCallbackUrl`].
    pub async fn new(
        addr: &GrpcCallbackUrl,
    ) -> Result<Self, CallbackClientError> {
        let addr = addr.addr();
        let client =
            Rc::new(RefCell::new(ProtoCallbackClient::connect(addr).await?));

        Ok(Self { client })
    }
}
