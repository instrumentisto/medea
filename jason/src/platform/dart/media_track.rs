//! Wrapper around [MediaStreamTrack][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams#mediastreamtrack

use derive_more::AsRef;

use crate::media::{track::MediaStreamTrackState, FacingMode, MediaKind};

/// Wrapper around [MediaStreamTrack][1] received from a
/// [getUserMedia()][2]/[getDisplayMedia()][3] request.
///
/// [1]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
/// [2]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
/// [3]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[derive(AsRef, Debug)]
pub struct MediaStreamTrack;

impl MediaStreamTrack {
    /// Returns [`id`] of the underlying [MediaStreamTrack][2].
    ///
    /// [`id`]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn id(&self) -> String {
        unimplemented!()
    }

    /// Returns this [`MediaStreamTrack`]'s kind (audio/video).
    #[inline]
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        unimplemented!()
    }

    /// Returns [MediaStreamTrackState][1] of the underlying
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrackstate
    /// [2]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    #[must_use]
    pub fn ready_state(&self) -> MediaStreamTrackState {
        unimplemented!()
    }

    /// Returns a [`deviceId`][1] of the underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediatracksettings-deviceid
    /// [2]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn device_id(&self) -> Option<String> {
        unimplemented!()
    }

    /// Return a [`facingMode`][1] of the underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediatracksettings-facingmode
    /// [2]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    #[must_use]
    pub fn facing_mode(&self) -> Option<FacingMode> {
        unimplemented!()
    }

    /// Returns a [`height`][1] of the underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediatracksettings-height
    /// [2]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn height(&self) -> Option<u32> {
        unimplemented!()
    }

    /// Return a [`width`][1] of the underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#dom-mediatracksettings-width
    /// [2]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn width(&self) -> Option<u32> {
        unimplemented!()
    }

    /// Changes an [`enabled`][1] attribute in the underlying
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        unimplemented!()
    }

    /// Changes a [`readyState`][1] attribute in the underlying
    /// [MediaStreamTrack][2] to [`ended`][3].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-readystate
    /// [2]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    /// [3]: https://tinyurl.com/w3-streams#idl-def-MediaStreamTrackState.ended
    #[inline]
    pub fn stop(&self) {
        unimplemented!()
    }

    /// Returns an [`enabled`][1] attribute of the underlying
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn enabled(&self) -> bool {
        unimplemented!()
    }

    /// Detects whether a video track captured from display searching
    /// [specific fields][1] in its settings.
    ///
    /// Only works in Chrome browser at the moment.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#extensions-to-mediatracksettings
    #[must_use]
    pub fn guess_is_from_display(&self) -> bool {
        unimplemented!()
    }

    /// Forks this [`MediaStreamTrack`].
    ///
    /// Creates a new [`MediaStreamTrack`] from this [`MediaStreamTrack`] using
    /// a [`clone()`][1] method. It won't clone current [`MediaStreamTrack`]'s
    /// callbacks.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamtrack-clone
    #[must_use]
    pub fn fork(&self) -> Self {
        unimplemented!()
    }

    /// Sets handler for the [`ended`][1] event.
    ///
    /// [1]: https://tinyurl.com/w3-streams#event-mediastreamtrack-ended
    #[allow(clippy::unused_self, clippy::needless_pass_by_value)]
    pub fn on_ended<F>(&self, f: Option<F>)
    where
        F: 'static + FnOnce(),
    {
        unimplemented!()
    }
}
