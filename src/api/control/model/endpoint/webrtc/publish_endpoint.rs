use crate::api::control::serde::endpoint::P2pMode;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize; // TODO: temp

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

pub use Id as WebRtcPublishId;

pub trait WebRtcPublishEndpoint {
    fn p2p(&self) -> &P2pMode;
}
