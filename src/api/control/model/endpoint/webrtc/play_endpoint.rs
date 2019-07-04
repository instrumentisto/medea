use crate::api::control::{
    endpoint::SerdeSrcUri,
    model::{
        endpoint::webrtc::publish_endpoint::WebRtcPublishId, member::MemberId,
        room::RoomId,
    },
};
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize;

use std::fmt::Debug;
pub use Id as WebRtcPlayId;
macro_attr! {
    /// ID of [`Room`].
    #[derive(
        Clone,
        Debug,
        Deserialize,
        Eq,
        Hash,
        PartialEq,
        NewtypeFrom!,
        NewtypeDisplay!,
    )]
    pub struct Id(pub String);
}

pub trait WebRtcPlayEndpoint {
    fn src(&self) -> &SerdeSrcUri;
}
