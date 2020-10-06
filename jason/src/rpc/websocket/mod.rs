mod client;
mod transport;

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;
#[doc(inline)]
pub use self::{
    client::{
        ClientDisconnect, RpcEvent, RpcTransportFactory, WebSocketRpcClient,
    },
    transport::{
        RpcTransport, TransportError, TransportState, WebSocketRpcTransport,
    },
};
