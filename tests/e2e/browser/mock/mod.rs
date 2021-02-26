pub mod gum;
pub mod websocket;

use super::Window;

pub use gum::Gum;
pub use websocket::WebSocket;

pub async fn instantiate_mocks(window: &Window) {
    WebSocket::instantiate(window).await;
    Gum::instantiate(window).await;
}

impl Window {
    pub fn websocket_mock(&self) -> WebSocket {
        WebSocket(self)
    }

    pub fn gum_mock(&self) -> Gum {
        Gum(self)
    }
}
