pub mod play_endpoint;
pub mod publish_endpoint;

pub use crate::api::control::serde::endpoint::SerdeSrcUri as SrcUri;
pub use play_endpoint::{WebRtcPlayEndpoint, WebRtcPlayId};
pub use publish_endpoint::{P2pMode, WebRtcPublishEndpoint, WebRtcPublishId};
