//! Implementations of the WebAPI objects mocks.

mod websocket;

use super::Window;

#[doc(inline)]
pub use websocket::WebSocket;

/// Instantiates all required mocks in the provided [`Window`].
pub async fn instantiate_mocks(window: &Window) {
    WebSocket::instantiate(window).await;
}

impl Window {
    /// Returns `WebSocket` object mock for this [`Window`].
    pub fn websocket_mock(&self) -> WebSocket {
        WebSocket(self)
    }
}
