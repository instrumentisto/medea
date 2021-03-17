//! Implementation of gRPC client for sending [`CallbackRequest`]s.

use std::fmt;

#[rustfmt::skip]
use medea_control_api_proto::grpc::callback::{
    callback_client::CallbackClient as ProtoCallbackClient
};
use async_trait::async_trait;
use tonic::transport::Channel;

use crate::api::control::callback::{
    clients::{CallbackClient, CallbackClientError},
    url::GrpcCallbackUrl,
    CallbackRequest,
};

/// gRPC client for sending [`CallbackRequest`]s.
pub struct GrpcCallbackClient {
    /// [`tonic`] gRPC client of Control API Callback service.
    client: ProtoCallbackClient<Channel>,
}

impl GrpcCallbackClient {
    /// Returns gRPC client for provided [`GrpcCallbackUrl`].
    ///
    /// Note that this function doesn't check availability of gRPC server on
    /// provided [`GrpcCallbackUrl`].
    ///
    /// # Errors
    ///
    /// With [`CallbackClientError::TonicTransport`] if [`tonic`] transport
    /// cannot be created (so gRPC connection cannot be established).
    pub async fn new(
        addr: &GrpcCallbackUrl,
    ) -> Result<Self, CallbackClientError> {
        let addr = addr.addr();
        let client = ProtoCallbackClient::connect(addr).await?;
        Ok(Self { client })
    }
}

#[async_trait(?Send)]
impl CallbackClient for GrpcCallbackClient {
    async fn send(
        &self,
        request: CallbackRequest,
    ) -> Result<(), CallbackClientError> {
        let mut client = self.client.clone();
        client.on_event(tonic::Request::new(request.into())).await?;

        Ok(())
    }
}

impl fmt::Debug for GrpcCallbackClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("GrpcCallbackClient")
            .field("client", &"/* Cannot be printed */")
            .finish()
    }
}

#[cfg(test)]
pub mod test {
    use std::net::ToSocketAddrs;

    use actix::Arbiter;
    use futures::{channel::oneshot, task, FutureExt as _, TryFutureExt as _};
    use medea_control_api_proto::grpc::callback::{
        callback_server::{Callback, CallbackServer as TonicCallbackServer},
        on_leave::Reason,
        request::Event,
        Request, Response,
    };
    use tonic::{transport::Server, Status};

    #[mockall::automock]
    pub trait GrpcCallbackServer {
        fn on_join(&self, fid: &str) -> Result<(), ()>;
        fn on_leave(&self, fid: &str, event: Reason) -> Result<(), ()>;
    }

    #[async_trait::async_trait]
    impl Callback for MockGrpcCallbackServer {
        async fn on_event(
            &self,
            request: tonic::Request<Request>,
        ) -> Result<tonic::Response<Response>, Status> {
            let request = request.into_inner();
            match request.event.unwrap() {
                Event::OnJoin(_) => self.on_join(&request.fid),
                Event::OnLeave(on_leave) => self.on_leave(
                    &request.fid,
                    Reason::from_i32(on_leave.reason).unwrap(),
                ),
            }
            .map(|_| tonic::Response::new(Response {}))
            .map_err(|_| Status::internal(""))
        }
    }

    /// Callback gRPC server close handle.
    pub struct CloseHandle(oneshot::Sender<()>);

    /// Starts [`CallbackServer`] registering provided mock as callback handler.
    pub async fn start_callback_server<A: ToSocketAddrs>(
        addr: A,
        callback: MockGrpcCallbackServer,
    ) -> CloseHandle {
        let (close_tx, close_rx) = oneshot::channel();
        let addr = addr.to_socket_addrs().unwrap().next().unwrap();

        let server = Server::builder()
            .add_service(TonicCallbackServer::new(callback))
            .serve_with_shutdown(addr, async move {
                close_rx.await.ok();
            })
            .map_err(|err| err.to_string())
            .shared();

        // Poll server here to proc socket binding to make sure that server is
        // ready when this function returns.
        if let task::Poll::Ready(maybe_err) =
            futures::poll!(server.clone().boxed())
        {
            maybe_err.unwrap();
        }

        Arbiter::spawn(async move {
            server.await.unwrap();
        });
        CloseHandle(close_tx)
    }
}
