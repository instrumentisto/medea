//! WebRTC related endpoints.

pub mod play_endpoint;
pub mod publish_endpoint;

use crate::api::control::callback::MediaType;

#[doc(inline)]
pub use play_endpoint::WebRtcPlayEndpoint;
#[doc(inline)]
pub use publish_endpoint::WebRtcPublishEndpoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MuteState(u8);

impl MuteState {
    #[inline]
    pub const fn new() -> MuteState {
        Self(0)
    }

    #[inline]
    pub const fn with_media_type(media_type: MediaType) -> MuteState {
        Self(media_type as u8)
    }

    #[inline]
    pub fn started(&mut self, media_type: MediaType) {
        self.0 |= media_type as u8;
    }

    #[inline]
    pub fn stopped(&mut self, media_type: MediaType) {
        self.0 &= !(media_type as u8);
    }

    #[inline]
    pub const fn is_started(self, media_type: MediaType) -> bool {
        let media_type = media_type as u8;
        (self.0 & media_type) == media_type
    }

    #[inline]
    pub const fn is_stopped(self, media_type: MediaType) -> bool {
        let media_type = !(media_type as u8);

        (self.0 | media_type) == media_type
    }
}

#[cfg(test)]
mod tracks_state_tests {
    use super::*;

    #[test]
    fn normally_sets_started() {
        let mut state = MuteState::new();

        assert!(!state.is_started(MediaType::Audio));
        assert!(!state.is_started(MediaType::Video));
        assert!(!state.is_started(MediaType::Both));

        state.started(MediaType::Audio);
        assert!(state.is_started(MediaType::Audio));
        assert!(!state.is_started(MediaType::Video));
        assert!(!state.is_started(MediaType::Both));

        state.started(MediaType::Video);
        assert!(state.is_started(MediaType::Video));
        assert!(state.is_started(MediaType::Audio));
        assert!(state.is_started(MediaType::Both));
    }

    #[test]
    fn normally_sets_started_on_both() {
        let mut state = MuteState::new();

        assert!(!state.is_started(MediaType::Video));
        assert!(!state.is_started(MediaType::Audio));
        assert!(!state.is_started(MediaType::Both));

        state.started(MediaType::Both);
        assert!(state.is_started(MediaType::Video));
        assert!(state.is_started(MediaType::Audio));
        assert!(state.is_started(MediaType::Both));
    }

    #[test]
    fn normally_sets_stopped() {
        let mut state = MuteState::new();
        state.started(MediaType::Both);
        assert!(state.is_started(MediaType::Video));
        assert!(state.is_started(MediaType::Audio));
        assert!(state.is_started(MediaType::Both));

        state.stopped(MediaType::Audio);
        assert!(!state.is_started(MediaType::Audio));
        assert!(state.is_started(MediaType::Video));
        assert!(!state.is_started(MediaType::Both));

        state.stopped(MediaType::Video);
        assert!(!state.is_started(MediaType::Audio));
        assert!(!state.is_started(MediaType::Video));
        assert!(!state.is_started(MediaType::Both));
    }

    #[test]
    fn normally_sets_stopped_on_both() {
        let mut state = MuteState::new();
        state.started(MediaType::Both);
        assert!(state.is_started(MediaType::Video));
        assert!(state.is_started(MediaType::Audio));
        assert!(state.is_started(MediaType::Both));

        state.stopped(MediaType::Both);
        assert!(!state.is_started(MediaType::Video));
        assert!(!state.is_started(MediaType::Audio));
        assert!(!state.is_started(MediaType::Both));
    }

    #[test]
    fn normally_works_is_stopped() {
        let mut state = MuteState::new();
        assert!(state.is_stopped(MediaType::Both));

        state.started(MediaType::Audio);
        assert!(state.is_stopped(MediaType::Video));
        assert!(!state.is_stopped(MediaType::Both));
        assert!(!state.is_stopped(MediaType::Audio));

        state.started(MediaType::Video);
        assert!(!state.is_stopped(MediaType::Video));
        assert!(!state.is_stopped(MediaType::Audio));
        assert!(!state.is_stopped(MediaType::Both));

        state.stopped(MediaType::Both);
        assert!(state.is_stopped(MediaType::Video));
        assert!(state.is_stopped(MediaType::Audio));
        assert!(state.is_stopped(MediaType::Both));
    }
}
