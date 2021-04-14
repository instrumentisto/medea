use dart_sys::Dart_Handle;
use futures::prelude::stream::LocalBoxStream;
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;

use crate::{
    platform::{RpcTransport, TransportError, TransportState},
    rpc::{ApiUrl, ClientDisconnect},
};

type Result<T, E = Traced<TransportError>> = std::result::Result<T, E>;

#[derive(Clone, Debug)]
pub struct WebSocketRpcTransport(Dart_Handle);

impl WebSocketRpcTransport {
    pub async fn new(url: ApiUrl) -> Result<Self> {
        todo!()
    }
}

impl RpcTransport for WebSocketRpcTransport {
    fn on_message(&self) -> LocalBoxStream<'static, ServerMsg> {
        todo!()
    }

    fn set_close_reason(&self, reason: ClientDisconnect) {
        todo!()
    }

    fn send(&self, msg: &ClientMsg) -> Result<(), Traced<TransportError>> {
        todo!()
    }

    fn on_state_change(&self) -> LocalBoxStream<'static, TransportState> {
        todo!()
    }
}
