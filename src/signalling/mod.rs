//! [WebRTC] [signalling] related implementations.
//!
//! [WebRTC]: https://webrtcglossary.com/webrtc/
//! [signalling]: https://webrtcglossary.com/signaling/

pub(crate) mod elements;
pub(crate) mod participants;
pub(crate) mod peers;
pub(crate) mod room;
pub(crate) mod room_repo;
pub(crate) mod room_service;

#[doc(inline)]
pub(crate) use self::room::Room;
#[doc(inline)]
pub use self::{room_repo::RoomRepository, room_service::RoomService};
