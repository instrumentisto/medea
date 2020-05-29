//! Implementation of the [`MediaTrafficState`] which will be used for
//! the storing started/stopped [`MediaType`]s.

use crate::api::control::callback::MediaType;

/// Traffic state of all [`MediaType`]s for some `Endpoint`.
///
/// All [`MediaType`]s can be in started or stopped state.
///
/// `1` bit in this bitflags structure represents that [`MediaType`] is started.
/// `0` bit in this bitflags structure represents that [`MediaType`] is stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaTrafficState(u8);

impl MediaTrafficState {
    /// Creates new [`MediaTrafficState`] in which all [`MediaType`]s is
    /// stopped.
    #[inline]
    pub const fn new() -> MediaTrafficState {
        Self(0)
    }

    /// Returns new [`MediaTrafficState`] in which provided [`MediaType`] is
    /// started.
    #[cfg(test)]
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
    pub fn get_started_media_type(self) -> Option<MediaType> {
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

/// Returns [`MediaType`] which was started based on [`MediaTrafficState`]
/// before and [`MediaTrafficState`] after. Returns `Some(MediaType)` if `after`
/// contains [`MediaType`] that is not present in `before` and `None` otherwise.
///
/// `None` will be returned if none of the [`MediaType`]s was started.
pub fn get_diff_added(
    before: MediaTrafficState,
    after: MediaTrafficState,
) -> Option<MediaType> {
    MediaTrafficState(!before.0 & after.0).get_started_media_type()
}

/// Returns [`MediaType`] which was stopped based on [`MediaTrafficState`]
/// before and [`MediaTrafficState`] after. Returns `Some(MediaType)` if
/// `before` contains [`MediaType`] that is not present in `after` and `None`
/// otherwise.
pub fn get_diff_removed(
    before: MediaTrafficState,
    after: MediaTrafficState,
) -> Option<MediaType> {
    MediaTrafficState(before.0 & !after.0).get_started_media_type()
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

    #[test]
    fn diff_started() {
        // Audio - 0b1
        // Video - 0b10
        // Both - 0b11

        // 0b1 -> 0b10 = 0b10
        let state_before = MediaTrafficState::with_media_type(MediaType::Audio);
        let state_after = MediaTrafficState::with_media_type(MediaType::Video);
        let started_media_type =
            get_diff_added(state_before, state_after).unwrap();
        assert_eq!(started_media_type, MediaType::Video);

        // 0b0 -> 0b1 = 0b1
        let state_before = MediaTrafficState::new();
        let state_after = MediaTrafficState::with_media_type(MediaType::Audio);
        let started_media_type =
            get_diff_added(state_before, state_after).unwrap();
        assert_eq!(started_media_type, MediaType::Audio);

        // 0b0 -> 0b11 = 0b11
        let state_before = MediaTrafficState::new();
        let state_after = MediaTrafficState::with_media_type(MediaType::Both);
        let started_media_type =
            get_diff_added(state_before, state_after).unwrap();
        assert_eq!(started_media_type, MediaType::Both);

        // 0b11 -> 0b0 = 0b0
        let state_before = MediaTrafficState::with_media_type(MediaType::Both);
        let state_after = MediaTrafficState::new();
        assert!(get_diff_added(state_before, state_after).is_none());

        // 0b11 -> 0b11 = 0b0
        let state_before = MediaTrafficState::with_media_type(MediaType::Both);
        let state_after = MediaTrafficState::with_media_type(MediaType::Both);
        assert!(get_diff_added(state_before, state_after).is_none());

        // 0b10 -> 0b1 = 0b1
        let state_before = MediaTrafficState::with_media_type(MediaType::Video);
        let state_after = MediaTrafficState::with_media_type(MediaType::Audio);
        assert_eq!(
            get_diff_added(state_before, state_after).unwrap(),
            MediaType::Audio
        );

        // 0b0 -> 0b10 = 0b10
        let state_before = MediaTrafficState::new();
        let state_after = MediaTrafficState::with_media_type(MediaType::Video);
        assert_eq!(
            get_diff_added(state_before, state_after).unwrap(),
            MediaType::Video
        );

        let state_before = MediaTrafficState::with_media_type(MediaType::Audio);
        let state_after = MediaTrafficState::with_media_type(MediaType::Both);
        assert_eq!(
            get_diff_added(state_before, state_after).unwrap(),
            MediaType::Video,
        );

        let state_before = MediaTrafficState::with_media_type(MediaType::Video);
        let state_after = MediaTrafficState::with_media_type(MediaType::Both);
        assert_eq!(
            get_diff_added(state_before, state_after).unwrap(),
            MediaType::Audio,
        );
    }

    #[test]
    fn diff_stopped() {
        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::with_media_type(MediaType::Audio);
        assert_eq!(get_diff_removed(before, after).unwrap(), MediaType::Video);

        let before = MediaTrafficState::with_media_type(MediaType::Audio);
        let after = MediaTrafficState::with_media_type(MediaType::Video);
        assert_eq!(get_diff_removed(before, after).unwrap(), MediaType::Audio);

        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::new();
        assert_eq!(get_diff_removed(before, after).unwrap(), MediaType::Both);

        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::with_media_type(MediaType::Both);
        assert!(get_diff_removed(before, after).is_none());

        let before = MediaTrafficState::new();
        let after = MediaTrafficState::new();
        assert!(get_diff_removed(before, after).is_none());

        let before = MediaTrafficState::new();
        let after = MediaTrafficState::with_media_type(MediaType::Both);
        assert!(get_diff_removed(before, after).is_none());

        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::with_media_type(MediaType::Audio);
        assert_eq!(get_diff_removed(before, after).unwrap(), MediaType::Video);

        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::with_media_type(MediaType::Video);
        assert_eq!(get_diff_removed(before, after).unwrap(), MediaType::Audio);
    }
}
