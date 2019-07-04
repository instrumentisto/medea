use crate::api::control::model::{
    endpoint::webrtc::publish_endpoint::WebRtcPublishId, member::MemberId,
    room::RoomId,
};
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize;

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

pub trait SrcUri {
    fn room_id(&self) -> &RoomId;

    fn member_id(&self) -> &MemberId;

    fn endpoint_id(&self) -> &WebRtcPublishId;
}

pub trait WebRtcPlayEndpoint {
    fn src(&self) -> Box<&dyn SrcUri>;
}
