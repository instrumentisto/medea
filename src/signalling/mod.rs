//! [WebRTC] [signalling] related implementations.
//!
//! [WebRTC]: https://webrtcglossary.com/webrtc/
//! [signalling]: https://webrtcglossary.com/signaling/

pub mod elements;
pub mod participants;
pub mod peers;
pub mod room;
pub mod room_repo;
pub mod room_service;

#[doc(inline)]
pub use self::room::Room;
