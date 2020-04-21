//! WebRTC related endpoints.

pub mod play_endpoint;
pub mod publish_endpoint;

#[doc(inline)]
pub use play_endpoint::WebRtcPlayEndpoint;
#[doc(inline)]
pub use publish_endpoint::WebRtcPublishEndpoint;

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
