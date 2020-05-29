//! Implementation of the [`MediaTrafficState`] which will be used for
//! the storing enabled/disabled [`MediaType`]s.

use crate::api::control::callback::MediaType;

/// Traffic state of all [`MediaType`]s for some `Endpoint`.
///
/// All [`MediaType`]s can be in enabled or disabled state.
///
/// `1` bit in this bitflags structure represents that [`MediaType`] is enabled.
/// `0` bit in this bitflags structure represents that [`MediaType`] is
/// disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaTrafficState(u8);

impl MediaTrafficState {
    /// Creates new [`MediaTrafficState`] in which all [`MediaType`]s is
    /// disabled.
    #[inline]
    pub const fn new() -> MediaTrafficState {
        Self(0)
    }

    /// Returns new [`MediaTrafficState`] in which provided [`MediaType`] is
    /// enabled.
    #[inline]
    pub const fn with_media_type(media_type: MediaType) -> MediaTrafficState {
        Self(media_type as u8)
    }

    /// Sets provided [`MediaType`] to the enabled state.
    ///
    /// Note that [`MediaType::Both`] will set [`MediaType::Audio`] and
    /// [`MediaType::Video`] to enabled state.
    #[inline]
    pub fn started(&mut self, media_type: MediaType) {
        self.0 |= media_type as u8;
    }

    /// Sets provided [`MediaType`] to the disabled state.
    ///
    /// Note that [`MediaType::Both`] will set [`MediaType::Audio`] and
    /// [`MediaType::Video`] to disabled state.
    #[inline]
    pub fn disable(&mut self, media_type: MediaType) {
        self.0 &= !(media_type as u8);
    }

    /// Returns `true` if the provided [`MediaType`] is enabled.
    ///
    /// Note that [`MediaType::Both`] will return `true` only if
    /// [`MediaType::Audio`] and [`MediaType::Video`] is enabled.
    #[inline]
    pub const fn is_enabled(self, media_type: MediaType) -> bool {
        let media_type = media_type as u8;

        (self.0 & media_type) == media_type
    }

    /// Returns `true` if the provided [`MediaType`] is disabled.
    ///
    /// Note that [`MediaType::Both`] will return `true` only if
    /// [`MediaType::Audio`] and [`MediaType::Video`] is disabled.
    #[inline]
    pub const fn is_disabled(self, media_type: MediaType) -> bool {
        let media_type = !(media_type as u8);

        (self.0 | media_type) == media_type
    }

    /// Returns [`MediaType`] which enabled according to this
    /// [`MediaTrafficState`].
    ///
    /// Returns `None` if all [`MediaType`]s disabled.
    pub fn get_enabled_media_type(self) -> Option<MediaType> {
        if self.is_enabled(MediaType::Both) {
            Some(MediaType::Both)
        } else if self.is_enabled(MediaType::Audio) {
            Some(MediaType::Audio)
        } else if self.is_enabled(MediaType::Video) {
            Some(MediaType::Video)
        } else {
            None
        }
    }
}

/// Returns [`MediaType`] which was enabled based on [`MediaTrafficState`]
/// before and [`MediaTrafficState`] after. Returns `Some(MediaType)` if `after`
/// contains [`MediaType`] that is not present in `before` and `None` otherwise.
///
/// `None` will be returned if none of the [`MediaType`]s was enabled.
pub fn get_diff_enabled(
    before: MediaTrafficState,
    after: MediaTrafficState,
) -> Option<MediaType> {
    MediaTrafficState(!before.0 & after.0).get_enabled_media_type()
}

/// Returns [`MediaType`] which was disabled based on [`MediaTrafficState`]
/// before and [`MediaTrafficState`] after.
///
/// Returns `Some(MediaType)` if `before` contains [`MediaType`] that is not
/// present in `after` and `None` otherwise.
pub fn get_diff_disabled(
    before: MediaTrafficState,
    after: MediaTrafficState,
) -> Option<MediaType> {
    MediaTrafficState(before.0 & !after.0).get_enabled_media_type()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normally_sets_enabled() {
        let mut state = MediaTrafficState::new();

        assert!(!state.is_enabled(MediaType::Audio));
        assert!(!state.is_enabled(MediaType::Video));
        assert!(!state.is_enabled(MediaType::Both));

        state.started(MediaType::Audio);
        assert!(state.is_enabled(MediaType::Audio));
        assert!(!state.is_enabled(MediaType::Video));
        assert!(!state.is_enabled(MediaType::Both));

        state.started(MediaType::Video);
        assert!(state.is_enabled(MediaType::Video));
        assert!(state.is_enabled(MediaType::Audio));
        assert!(state.is_enabled(MediaType::Both));
    }

    #[test]
    fn normally_sets_both_enabled() {
        let mut state = MediaTrafficState::new();

        assert!(!state.is_enabled(MediaType::Video));
        assert!(!state.is_enabled(MediaType::Audio));
        assert!(!state.is_enabled(MediaType::Both));

        state.started(MediaType::Both);
        assert!(state.is_enabled(MediaType::Video));
        assert!(state.is_enabled(MediaType::Audio));
        assert!(state.is_enabled(MediaType::Both));
    }

    #[test]
    fn normally_sets_disabled() {
        let mut state = MediaTrafficState::new();
        state.started(MediaType::Both);
        assert!(state.is_enabled(MediaType::Video));
        assert!(state.is_enabled(MediaType::Audio));
        assert!(state.is_enabled(MediaType::Both));

        state.disable(MediaType::Audio);
        assert!(!state.is_enabled(MediaType::Audio));
        assert!(state.is_enabled(MediaType::Video));
        assert!(!state.is_enabled(MediaType::Both));

        state.disable(MediaType::Video);
        assert!(!state.is_enabled(MediaType::Audio));
        assert!(!state.is_enabled(MediaType::Video));
        assert!(!state.is_enabled(MediaType::Both));
    }

    #[test]
    fn normally_sets_both_disabled() {
        let mut state = MediaTrafficState::new();
        state.started(MediaType::Both);
        assert!(state.is_enabled(MediaType::Video));
        assert!(state.is_enabled(MediaType::Audio));
        assert!(state.is_enabled(MediaType::Both));

        state.disable(MediaType::Both);
        assert!(!state.is_enabled(MediaType::Video));
        assert!(!state.is_enabled(MediaType::Audio));
        assert!(!state.is_enabled(MediaType::Both));
    }

    #[test]
    fn normally_works_is_disabled() {
        let mut state = MediaTrafficState::new();
        assert!(state.is_disabled(MediaType::Both));

        state.started(MediaType::Audio);
        assert!(state.is_disabled(MediaType::Video));
        assert!(!state.is_disabled(MediaType::Both));
        assert!(!state.is_disabled(MediaType::Audio));

        state.started(MediaType::Video);
        assert!(!state.is_disabled(MediaType::Video));
        assert!(!state.is_disabled(MediaType::Audio));
        assert!(!state.is_disabled(MediaType::Both));

        state.disable(MediaType::Both);
        assert!(state.is_disabled(MediaType::Video));
        assert!(state.is_disabled(MediaType::Audio));
        assert!(state.is_disabled(MediaType::Both));
    }

    #[test]
    fn diff_enabled() {
        // Audio - 0b1
        // Video - 0b10
        // Both - 0b11

        // 0b1 -> 0b10 = 0b10
        let state_before = MediaTrafficState::with_media_type(MediaType::Audio);
        let state_after = MediaTrafficState::with_media_type(MediaType::Video);
        let started_media_type =
            get_diff_enabled(state_before, state_after).unwrap();
        assert_eq!(started_media_type, MediaType::Video);

        // 0b0 -> 0b1 = 0b1
        let state_before = MediaTrafficState::new();
        let state_after = MediaTrafficState::with_media_type(MediaType::Audio);
        let started_media_type =
            get_diff_enabled(state_before, state_after).unwrap();
        assert_eq!(started_media_type, MediaType::Audio);

        // 0b0 -> 0b11 = 0b11
        let state_before = MediaTrafficState::new();
        let state_after = MediaTrafficState::with_media_type(MediaType::Both);
        let started_media_type =
            get_diff_enabled(state_before, state_after).unwrap();
        assert_eq!(started_media_type, MediaType::Both);

        // 0b11 -> 0b0 = 0b0
        let state_before = MediaTrafficState::with_media_type(MediaType::Both);
        let state_after = MediaTrafficState::new();
        assert!(get_diff_enabled(state_before, state_after).is_none());

        // 0b11 -> 0b11 = 0b0
        let state_before = MediaTrafficState::with_media_type(MediaType::Both);
        let state_after = MediaTrafficState::with_media_type(MediaType::Both);
        assert!(get_diff_enabled(state_before, state_after).is_none());

        // 0b10 -> 0b1 = 0b1
        let state_before = MediaTrafficState::with_media_type(MediaType::Video);
        let state_after = MediaTrafficState::with_media_type(MediaType::Audio);
        assert_eq!(
            get_diff_enabled(state_before, state_after).unwrap(),
            MediaType::Audio
        );

        // 0b0 -> 0b10 = 0b10
        let state_before = MediaTrafficState::new();
        let state_after = MediaTrafficState::with_media_type(MediaType::Video);
        assert_eq!(
            get_diff_enabled(state_before, state_after).unwrap(),
            MediaType::Video
        );

        let state_before = MediaTrafficState::with_media_type(MediaType::Audio);
        let state_after = MediaTrafficState::with_media_type(MediaType::Both);
        assert_eq!(
            get_diff_enabled(state_before, state_after).unwrap(),
            MediaType::Video,
        );

        let state_before = MediaTrafficState::with_media_type(MediaType::Video);
        let state_after = MediaTrafficState::with_media_type(MediaType::Both);
        assert_eq!(
            get_diff_enabled(state_before, state_after).unwrap(),
            MediaType::Audio,
        );
    }

    #[test]
    fn diff_disabled() {
        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::with_media_type(MediaType::Audio);
        assert_eq!(get_diff_disabled(before, after).unwrap(), MediaType::Video);

        let before = MediaTrafficState::with_media_type(MediaType::Audio);
        let after = MediaTrafficState::with_media_type(MediaType::Video);
        assert_eq!(get_diff_disabled(before, after).unwrap(), MediaType::Audio);

        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::new();
        assert_eq!(get_diff_disabled(before, after).unwrap(), MediaType::Both);

        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::with_media_type(MediaType::Both);
        assert!(get_diff_disabled(before, after).is_none());

        let before = MediaTrafficState::new();
        let after = MediaTrafficState::new();
        assert!(get_diff_disabled(before, after).is_none());

        let before = MediaTrafficState::new();
        let after = MediaTrafficState::with_media_type(MediaType::Both);
        assert!(get_diff_disabled(before, after).is_none());

        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::with_media_type(MediaType::Audio);
        assert_eq!(get_diff_disabled(before, after).unwrap(), MediaType::Video);

        let before = MediaTrafficState::with_media_type(MediaType::Both);
        let after = MediaTrafficState::with_media_type(MediaType::Video);
        assert_eq!(get_diff_disabled(before, after).unwrap(), MediaType::Audio);
    }
}
