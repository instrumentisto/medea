//! WebAPI objects mocks.

pub mod gum;
pub mod websocket;

use super::Window;

#[doc(inline)]
pub use self::{gum::Gum, websocket::WebSocket};

/// Instantiates all the required mocks in the provided [`Window`].
#[inline]
pub async fn instantiate_mocks(window: &Window) {
    WebSocket::instantiate(window).await;
    Gum::instantiate(window).await;
}

impl Window {
    /// Returns a `WebSocket` object mock for this [`Window`].
    #[inline]
    #[must_use]
    pub fn websocket_mock(&self) -> WebSocket {
        WebSocket(self)
    }

    /// Returns `MediaDevices.getUserMedia` function mock for this [`Window`].
    #[inline]
    #[must_use]
    pub fn gum_mock(&self) -> Gum {
        Gum(self)
    }
}
