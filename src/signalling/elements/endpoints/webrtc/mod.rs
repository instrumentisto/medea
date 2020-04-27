//! WebRTC related endpoints.

pub mod play_endpoint;
pub mod publish_endpoint;

use crate::api::control::callback::MediaType;

#[doc(inline)]
pub use play_endpoint::WebRtcPlayEndpoint;
#[doc(inline)]
pub use publish_endpoint::WebRtcPublishEndpoint;

/// Traffic state of all [`MediaType`]s for some `Endpoint`.
///
/// All [`MediaType`]s can be in started or stopped state.
///
/// If you wanna use this structure than you can just use it methods without
/// understanding how it works.
///
/// `1` bit in this bitflags structure represents that [`MediaType`] is started.
///
/// `0` bit in this bitflags structure represents that [`MediaType`] is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MediaTrafficState(u8);

impl MediaTrafficState {
    /// Creates new [`MediaTrafficState`] in which all [`MediaType`]s is
    /// stopped.
    #[inline]
    pub const fn new() -> MediaTrafficState {
        Self(0)
    }

    /// Returns new [`MediaTrafficState`] in which provided [`MediaType`] is
    /// started.
    #[inline]
    pub const fn with_media_type(media_type: MediaType) -> MediaTrafficState {
        Self(media_type as u8)
    }

    /// Sets provided [`MediaType`] to the started state.
    ///
    /// Note that [`MediaType::Both`] will set [`MediaType::Audio`] and
    /// [`MediaType::Video`] to started state.
    #[inline]
    pub fn started(&mut self, media_type: MediaType) {
        self.0 |= media_type as u8;
    }

    /// Sets provided [`MediaType`] to the stopped state.
    ///
    /// Note that [`MediaType::Both`] will set [`MediaType::Audio`] and
    /// [`MediaType::Video`] to stopped state.
    #[inline]
    pub fn stopped(&mut self, media_type: MediaType) {
        self.0 &= !(media_type as u8);
    }

    /// Returns `true` if the provided [`MediaType`] is started.
    ///
    /// Note that [`MediaType::Both`] will return `true` only if
    /// [`MediaType::Audio`] and [`MediaType::Video`] is started.
    #[inline]
    pub const fn is_started(self, media_type: MediaType) -> bool {
        let media_type = media_type as u8;
        (self.0 & media_type) == media_type
    }

    /// Returns `true` if the provided [`MediaType`] is stopped.
    ///
    /// Note that [`MediaType::Both`] will return `true` only if
    /// [`MediaType::Audio`] and [`MediaType::Video`] is stopped.
    #[inline]
    pub const fn is_stopped(self, media_type: MediaType) -> bool {
        let media_type = !(media_type as u8);

        (self.0 | media_type) == media_type
    }

    /// Returns [`MediaType`] which started according to this
    /// [`MediaTrafficState`].
    ///
    /// Returns `None` if all [`MediaType`]s stopped.
    pub fn into_media_type(self) -> Option<MediaType> {
        if self.is_started(MediaType::Both) {
            Some(MediaType::Both)
        } else if self.is_started(MediaType::Audio) {
            Some(MediaType::Audio)
        } else if self.is_started(MediaType::Video) {
            Some(MediaType::Video)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tracks_state_tests {
    use super::*;

    #[test]
    fn normally_sets_started() {
        let mut state = MediaTrafficState::new();

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
        let mut state = MediaTrafficState::new();

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
        let mut state = MediaTrafficState::new();
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
        let mut state = MediaTrafficState::new();
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
        let mut state = MediaTrafficState::new();
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
