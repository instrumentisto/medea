//! WebRTC related endpoints.

pub mod play_endpoint;
pub mod publish_endpoint;

#[doc(inline)]
pub use play_endpoint::{WebRtcPlayEndpoint, WebRtcPlayId};
#[doc(inline)]
pub use publish_endpoint::{WebRtcPublishEndpoint, WebRtcPublishId};
