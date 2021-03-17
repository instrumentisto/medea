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
        ClientDisconnect, ClientState, RpcEvent, RpcEventHandler,
        RpcTransportFactory, WebSocketRpcClient,
    },
    transport::{
        RpcTransport, TransportError, TransportState, WebSocketRpcTransport,
    },
};
