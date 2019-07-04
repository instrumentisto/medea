use crate::api::control::model::member::{MemberId, MemberSpec};
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

pub use Id as RoomId;

pub trait RoomSpec {
    fn members(&self) -> HashMap<&MemberId, Box<&dyn MemberSpec>>;

    fn id(&self) -> &Id;
}
