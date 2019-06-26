//! Signalling representation of control spec.

pub mod endpoint;
pub mod participant;

#[doc(inline)]
pub use self::participant::{Participant, ParticipantsLoadError, parse_participants};