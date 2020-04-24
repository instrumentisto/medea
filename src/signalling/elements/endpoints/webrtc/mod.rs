//! WebRTC related endpoints.

pub mod play_endpoint;
pub mod publish_endpoint;

use crate::api::control::callback::EndpointKind;

#[doc(inline)]
pub use play_endpoint::WebRtcPlayEndpoint;
#[doc(inline)]
pub use publish_endpoint::WebRtcPublishEndpoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TracksState(u8);

impl TracksState {
    #[inline]
    pub const fn new() -> TracksState {
        Self(0)
    }

    #[inline]
    pub const fn with_kind(kind: EndpointKind) -> TracksState {
        Self(kind as u8)
    }

    #[inline]
    pub fn started(&mut self, kind: EndpointKind) {
        self.0 |= kind as u8;
    }

    #[inline]
    pub fn stopped(&mut self, kind: EndpointKind) {
        self.0 &= !(kind as u8);
    }

    #[inline]
    pub const fn is_started(self, kind: EndpointKind) -> bool {
        let kind = kind as u8;
        (self.0 & kind) == kind
    }

    pub const fn is_stopped(self, kind: EndpointKind) -> bool {
        let kind = !(kind as u8);

        (self.0 | kind) == kind
    }
}

#[cfg(test)]
mod tracks_state_tests {
    use super::*;

    #[test]
    fn normally_sets_started() {
        let mut state = TracksState::new();

        assert!(!state.is_started(EndpointKind::Audio));
        assert!(!state.is_started(EndpointKind::Video));
        assert!(!state.is_started(EndpointKind::Both));

        state.started(EndpointKind::Audio);
        assert!(state.is_started(EndpointKind::Audio));
        assert!(!state.is_started(EndpointKind::Video));
        assert!(!state.is_started(EndpointKind::Both));

        state.started(EndpointKind::Video);
        assert!(state.is_started(EndpointKind::Video));
        assert!(state.is_started(EndpointKind::Audio));
        assert!(state.is_started(EndpointKind::Both));
    }

    #[test]
    fn normally_sets_started_on_both() {
        let mut state = TracksState::new();

        assert!(!state.is_started(EndpointKind::Video));
        assert!(!state.is_started(EndpointKind::Audio));
        assert!(!state.is_started(EndpointKind::Both));

        state.started(EndpointKind::Both);
        assert!(state.is_started(EndpointKind::Video));
        assert!(state.is_started(EndpointKind::Audio));
        assert!(state.is_started(EndpointKind::Both));
    }

    #[test]
    fn normally_sets_stopped() {
        let mut state = TracksState::new();
        state.started(EndpointKind::Both);
        assert!(state.is_started(EndpointKind::Video));
        assert!(state.is_started(EndpointKind::Audio));
        assert!(state.is_started(EndpointKind::Both));

        state.stopped(EndpointKind::Audio);
        assert!(!state.is_started(EndpointKind::Audio));
        assert!(state.is_started(EndpointKind::Video));
        assert!(!state.is_started(EndpointKind::Both));

        state.stopped(EndpointKind::Video);
        assert!(!state.is_started(EndpointKind::Audio));
        assert!(!state.is_started(EndpointKind::Video));
        assert!(!state.is_started(EndpointKind::Both));
    }

    #[test]
    fn normally_sets_stopped_on_both() {
        let mut state = TracksState::new();
        state.started(EndpointKind::Both);
        assert!(state.is_started(EndpointKind::Video));
        assert!(state.is_started(EndpointKind::Audio));
        assert!(state.is_started(EndpointKind::Both));

        state.stopped(EndpointKind::Both);
        assert!(!state.is_started(EndpointKind::Video));
        assert!(!state.is_started(EndpointKind::Audio));
        assert!(!state.is_started(EndpointKind::Both));
    }

    #[test]
    fn normally_works_is_stopped() {
        let mut state = TracksState::new();
        assert!(state.is_stopped(EndpointKind::Both));

        state.started(EndpointKind::Audio);
        assert!(state.is_stopped(EndpointKind::Video));
        assert!(!state.is_stopped(EndpointKind::Both));
        assert!(!state.is_stopped(EndpointKind::Audio));

        state.started(EndpointKind::Video);
        assert!(!state.is_stopped(EndpointKind::Video));
        assert!(!state.is_stopped(EndpointKind::Audio));
        assert!(!state.is_stopped(EndpointKind::Both));

        state.stopped(EndpointKind::Both);
        assert!(state.is_stopped(EndpointKind::Video));
        assert!(state.is_stopped(EndpointKind::Audio));
        assert!(state.is_stopped(EndpointKind::Both));
    }
}
