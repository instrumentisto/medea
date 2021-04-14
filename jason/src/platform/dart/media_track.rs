use dart_sys::Dart_Handle;

use crate::media::{track::MediaStreamTrackState, FacingMode, MediaKind};

#[derive(Clone, Debug)]
pub struct MediaStreamTrack(Dart_Handle);

impl MediaStreamTrack {
    pub fn id(&self) -> String {
        todo!()
    }

    pub fn kind(&self) -> MediaKind {
        todo!()
    }

    pub fn ready_state(&self) -> MediaStreamTrackState {
        todo!()
    }

    pub fn device_id(&self) -> Option<String> {
        todo!()
    }

    pub fn facing_mode(&self) -> Option<FacingMode> {
        todo!()
    }

    pub fn height(&self) -> Option<u32> {
        todo!()
    }

    pub fn width(&self) -> Option<u32> {
        todo!()
    }

    pub fn set_enabled(&self, enabled: bool) {
        todo!()
    }

    pub fn stop(&self) {
        todo!()
    }

    pub fn enabled(&self) -> bool {
        todo!()
    }

    pub fn guess_is_from_display(&self) -> bool {
        todo!()
    }

    pub fn fork(&self) -> Self {
        todo!()
    }

    pub fn on_ended<F>(&self, f: Option<F>)
    where
        F: 'static + FnOnce(),
    {
        todo!()
    }
}
