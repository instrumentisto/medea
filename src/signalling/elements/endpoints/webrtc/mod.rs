//! WebRTC related endpoints.

pub mod play_endpoint;
pub mod publish_endpoint;

use crate::api::control::callback::EndpointKind;
#[doc(inline)]
pub use play_endpoint::WebRtcPlayEndpoint;
#[doc(inline)]
pub use publish_endpoint::WebRtcPublishEndpoint;

#[derive(Debug, Clone, Copy)]
struct TracksState(u8);

impl TracksState {
    pub const fn new() -> TracksState {
        Self(0)
    }

    pub fn started(&mut self, kind: EndpointKind) {
        self.0 = self.0 | (kind as u8);
    }

    pub fn stopped(&mut self, kind: EndpointKind) {
        self.0 = self.0 & !(kind as u8);
    }

    pub const fn is_started(&self, kind: EndpointKind) -> bool {
        let kind = kind as u8;
        (self.0 & kind) == kind
    }
}

/// Publishing state of the WebRTC endpoints.
///
/// This state should change only on `on_start`/`on_stop` callback sending.
///
/// Theoretically this state also can change if no `on_start`/`on_stop`
/// callbacks was set but someone tries to get this kind of callbacks from
/// endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndpointState {
    /// Endpoint's `on_start` callback was sent.
    Started,

    /// Endpoint's `on_stop` callback was sent.
    Stopped,
}
