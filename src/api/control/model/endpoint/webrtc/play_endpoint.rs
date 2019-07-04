use crate::api::control::serde::endpoint::SerdeSrcUri;

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

pub trait WebRtcPlayEndpoint {
    fn src(&self) -> SerdeSrcUri;
}
