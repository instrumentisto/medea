//! Signalling representation of control spec.

pub mod member;

#[doc(inline)]
pub use self::member::{parse_members, Member, MembersLoadError};
