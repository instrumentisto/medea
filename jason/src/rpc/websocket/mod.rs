//! Implementation of the abstractions around [WebSocket] transport.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

mod client;
mod transport;

#[cfg(feature = "mockable")]
pub use self::transport::MockRpcTransport;
#[doc(inline)]
pub use self::{
    client::{
        ClientDisconnect, RpcEvent, RpcEventHandler, RpcTransportFactory,
        WebSocketRpcClient, ClientState
    },
    transport::{
        RpcTransport, TransportError, WebSocketRpcTransport, TransportState
    },
};
