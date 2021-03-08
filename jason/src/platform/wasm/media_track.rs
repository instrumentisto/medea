//! Wrapper around [MediaStreamTrack][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack

use derive_more::AsRef;

use crate::{
    media::{track::MediaStreamTrackState, FacingMode, MediaKind},
    platform::get_property_by_name,
};

/// Wrapper around [MediaStreamTrack][1] received from a
/// [getUserMedia()][2]/[getDisplayMedia()][3] request.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
/// [2]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [3]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[derive(AsRef, Clone, Debug)]
pub struct MediaStreamTrack {
    #[as_ref]
    sys_track: web_sys::MediaStreamTrack,
    kind: MediaKind,
}

impl<T> From<T> for MediaStreamTrack
where
    web_sys::MediaStreamTrack: From<T>,
{
    #[inline]
    fn from(from: T) -> MediaStreamTrack {
        let sys_track = web_sys::MediaStreamTrack::from(from);
        let kind = match sys_track.kind().as_ref() {
            "audio" => MediaKind::Audio,
            "video" => MediaKind::Video,
            _ => unreachable!(),
        };
        MediaStreamTrack { sys_track, kind }
    }
}

impl MediaStreamTrack {
    /// Returns [`id`] of the underlying [MediaStreamTrack][2].
    ///
    /// [`id`]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn id(&self) -> String {
        self.sys_track.id()
    }

    /// Returns this [`MediaStreamTrack`]'s kind (audio/video).
    #[inline]
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    /// Returns [MediaStreamTrackState][1] of the underlying
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrackstate
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[must_use]
    pub fn ready_state(&self) -> MediaStreamTrackState {
        let state = self.sys_track.ready_state();
        match state {
            web_sys::MediaStreamTrackState::Live => MediaStreamTrackState::Live,
            web_sys::MediaStreamTrackState::Ended => {
                MediaStreamTrackState::Ended
            }
            web_sys::MediaStreamTrackState::__Nonexhaustive => {
                unreachable!("Unknown MediaStreamTrackState::{:?}", state)
            }
        }
    }

    /// Returns a [`deviceId`][1] of the underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams/#dom-mediatracksettings-deviceid
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn device_id(&self) -> Option<String> {
        get_property_by_name(&self.sys_track.get_settings(), "deviceId", |v| {
            v.as_string()
        })
    }

    /// Return a [`facingMode`][1] of the underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams/#dom-mediatracksettings-facingmode
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[must_use]
    pub fn facing_mode(&self) -> Option<FacingMode> {
        let facing_mode = get_property_by_name(
            &self.sys_track.get_settings(),
            "facingMode",
            |v| v.as_string(),
        );
        facing_mode.and_then(|facing_mode| match facing_mode.as_ref() {
            "user" => Some(FacingMode::User),
            "environment" => Some(FacingMode::Environment),
            "left" => Some(FacingMode::Left),
            "right" => Some(FacingMode::Right),
            _ => {
                log::error!("Unknown FacingMode: {}", facing_mode);
                None
            }
        })
    }

    /// Returns a [`height`][1] of the underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams/#dom-mediatracksettings-height
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn height(&self) -> Option<u32> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        get_property_by_name(&self.sys_track.get_settings(), "height", |v| {
            v.as_f64().map(|v| v as u32)
        })
    }

    /// Return a [`width`][1] of the underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams/#dom-mediatracksettings-width
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn width(&self) -> Option<u32> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        get_property_by_name(&self.sys_track.get_settings(), "width", |v| {
            v.as_f64().map(|v| v as u32)
        })
    }

    /// Changes an [`enabled`][1] attribute in the underlying
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.sys_track.set_enabled(enabled);
    }

    /// Changes a [`readyState`][1] attribute in the underlying
    /// [MediaStreamTrack][2] to [`ended`][3].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-readystate
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    /// [3]: https://tinyurl.com/w3-streams#idl-def-MediaStreamTrackState.ended
    #[inline]
    pub fn stop(&self) {
        self.sys_track.stop()
    }

    /// Returns an [`enabled`][1] attribute of the underlying
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.sys_track.enabled()
    }

    /// Detects whether a video track captured from display searching
    /// [specific fields][1] in its settings.
    ///
    /// Only works in Chrome browser at the moment.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#extensions-to-mediatracksettings
    #[must_use]
    pub fn guess_is_from_display(&self) -> bool {
        let settings = self.sys_track.get_settings();

        let has_display_surface =
            get_property_by_name(&settings, "displaySurface", |val| {
                val.as_string()
            })
            .is_some();

        if has_display_surface {
            true
        } else {
            get_property_by_name(&settings, "logicalSurface", |val| {
                val.as_string()
            })
            .is_some()
        }
    }
}
