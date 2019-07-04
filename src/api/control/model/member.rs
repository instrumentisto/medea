use super::endpoint::webrtc::*;
use hashbrown::HashMap;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize;

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

pub use Id as MemberId;

pub trait MemberSpec {
    fn webrtc_play_endpoints(
        &self,
    ) -> HashMap<WebRtcPlayId, Box<&dyn WebRtcPlayEndpoint>>;

    fn webrtc_publish_endpoints(
        &self,
    ) -> HashMap<WebRtcPublishId, Box<&dyn WebRtcPublishEndpoint>>;

    fn credentials(&self) -> &String;

    fn id(&self) -> &MemberId;

    fn get_webrtc_play_by_id(
        &self,
        id: WebRtcPlayId,
    ) -> Option<Box<&dyn WebRtcPlayEndpoint>>;

    fn get_webrtc_publish_by_id(
        &self,
        id: WebRtcPublishId,
    ) -> Option<Box<&dyn WebRtcPublishEndpoint>>;
}
