pub mod websocket;

use super::Window;

pub use websocket::WebSocket;

pub async fn instantiate_mocks(window: &Window) {
    WebSocket::instantiate(window).await;
}

impl Window {
    pub fn websocket_mock(&self) -> WebSocket {
        WebSocket(self)
    }
}