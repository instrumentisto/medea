mod client;
mod transport;

#[doc(inline)]
pub use self::{
    client::{ClientDisconnect, RpcEvent, WebSocketRpcClient},
    transport::{RpcTransport, TransportError, WebSocketRpcTransport},
};
