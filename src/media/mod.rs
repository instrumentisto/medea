//! Representations of media and media connection establishment objects.

pub mod peer;
pub mod track;

#[doc(inline)]
pub use self::{
    peer::{
        Peer, PeerError, PeerStateMachine, Stable, WaitLocalSdp, WaitRemoteSdp,
    },
    track::MediaTrack,
};
