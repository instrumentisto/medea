//! Implementation of the abstractions around [WebSocket] transport.
//!
//! [WebSocket]: https://developer.mozilla.org/ru/docs/WebSockets

mod client;

#[doc(inline)]
pub use self::client::{
    ClientDisconnect, ClientState, RpcEvent, RpcEventHandler,
    RpcTransportFactory, WebSocketRpcClient,
};
