use crate::api::control::model::member::{MemberId, MemberSpec};
use hashbrown::HashMap;
use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};
use serde::Deserialize;


pub use Id as RoomId;

pub trait RoomSpec {
    fn members(&self) -> HashMap<MemberId, Box<dyn MemberSpec>>;

    fn id(&self) -> Id;

    fn get_member_by_id(&self, id: &MemberId) -> Option<Box<dyn MemberSpec>>;
}
