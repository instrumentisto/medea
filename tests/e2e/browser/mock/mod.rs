//! WebAPI objects mocks.

pub mod websocket;

use super::Window;

pub use self::websocket::WebSocket;

/// Instantiates all the required mocks in the provided [`Window`].
#[inline]
pub async fn instantiate_mocks(window: &Window) {
    WebSocket::instantiate(window).await;
}

impl Window {
    /// Returns a `WebSocket` object mock for this [`Window`].
    #[inline]
    #[must_use]
    pub fn websocket_mock(&self) -> WebSocket {
        WebSocket(self)
    }
}
