use serde_derive::Serialize;

/// Event is WebSocket messages sent by [`Media Server`] to [`Web Client`].
#[derive(Debug, Serialize)]
pub enum Event {
    #[serde(rename = "pong")]
    Pong(usize),
}
