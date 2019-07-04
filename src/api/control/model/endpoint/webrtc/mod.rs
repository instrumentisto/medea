pub mod play_endpoint;
pub mod publish_endpoint;

pub use crate::api::control::endpoint::SerdeSrcUri as SrcUri;
pub use play_endpoint::{WebRtcPlayEndpoint, WebRtcPlayId};
pub use publish_endpoint::{WebRtcPublishEndpoint, WebRtcPublishId};
