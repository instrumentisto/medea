//! Adapters to [`RTCPeerConnection`][1] and related objects.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

mod ice_server;
mod media_connections;
mod peer_con;
mod repo;

#[doc(inline)]
pub use self::{
    peer_con::{Id as PeerId, PeerConnection, PeerEvent, PeerEventHandler},
    repo::PeerRepository,
};
