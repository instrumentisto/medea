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

pub use Id as WebRtcPublishId;

pub enum P2pMode {
    Always,
}

pub trait WebRtcPublishEndpoint {
    fn p2p(&self) -> &P2pMode;
}
