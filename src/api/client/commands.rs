use actix::Message;
use actix_web::ws::CloseReason;
use serde_derive::Deserialize;

/// Command is WebSocket messages sent by [`Web Client`] to [`Media Server`].
#[derive(Debug, Message, Deserialize)]
pub enum Command {
    #[serde(rename = "ping")]
    Ping(usize),
    #[serde(rename = "close")]
    Close(Option<CloseReason>),
}
