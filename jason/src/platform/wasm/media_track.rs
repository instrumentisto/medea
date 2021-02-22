use derive_more::AsRef;

use crate::{api::FacingMode, core, platform::get_property_by_name};

#[derive(AsRef, Clone, Debug)]
pub struct MediaStreamTrack {
    #[as_ref]
    sys_track: web_sys::MediaStreamTrack,
    kind: core::MediaKind,
}

impl<T> From<T> for MediaStreamTrack
where
    web_sys::MediaStreamTrack: From<T>,
{
    #[inline]
    fn from(from: T) -> MediaStreamTrack {
        let sys_track = web_sys::MediaStreamTrack::from(from);
        let kind = match sys_track.kind().as_ref() {
            "audio" => core::MediaKind::Audio,
            "video" => core::MediaKind::Video,
            _ => unreachable!(),
        };

        MediaStreamTrack { sys_track, kind }
    }
}

impl MediaStreamTrack {
    pub fn id(&self) -> String {
        self.sys_track.id()
    }

    pub fn kind(&self) -> core::MediaKind {
        self.kind
    }

    pub fn ready_state(&self) -> web_sys::MediaStreamTrackState {
        self.sys_track.ready_state()
    }

    pub fn device_id(&self) -> Option<String> {
        get_property_by_name(&self.sys_track.get_settings(), "device_id", |v| {
            v.as_string()
        })
    }

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
                // TODO: log err
                None
            }
        })
    }

    pub fn height(&self) -> Option<u32> {
        get_property_by_name(&self.sys_track.get_settings(), "height", |v| {
            v.as_f64().map(|v| v as u32)
        })
    }

    pub fn width(&self) -> Option<u32> {
        get_property_by_name(&self.sys_track.get_settings(), "width", |v| {
            v.as_f64().map(|v| v as u32)
        })
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.sys_track.set_enabled(enabled);
    }

    pub fn stop(&self) {
        self.sys_track.stop()
    }

    pub fn enabled(&self) -> bool {
        self.sys_track.enabled()
    }

    /// Detects if video track captured from display searching [specific
    /// fields][1] in its settings. Only works in Chrome atm.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#extensions-to-mediatracksettings
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
