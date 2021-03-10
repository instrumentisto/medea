#![allow(
    clippy::new_without_default,
    clippy::module_name_repetitions,
    clippy::items_after_statements,
    clippy::wildcard_imports,
    clippy::must_use_candidate,
    clippy::unused_self,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::shadow_unrelated
)]

use android_logger::Config;
use log::{info, Level};

mod context;
mod jni;

use crate::jni::*;

pub use crate::jni::exec_foreign;
use std::sync::Arc;

pub enum FacingMode {
    User,
    Environment,
    Left,
    Right,
}

pub enum MediaKind {
    Audio,
    Video,
}

pub enum MediaSourceKind {
    Device,
    Display,
}

pub struct Jason;

impl Jason {
    pub fn new() -> Self {
        android_logger::init_once(
            Config::default()
                .with_min_level(Level::Trace)
                .with_tag("Jason"),
        );

        info!("Jason::new() {:?}", std::thread::current());
        Self
    }

    pub fn init_room(&self) -> RoomHandle {
        RoomHandle
    }

    pub fn media_manager(&self) -> MediaManagerHandle {
        MediaManagerHandle
    }

    pub fn close_room(&self, _room_to_delete: RoomHandle) {}

    pub fn dispose(&self) {}
}

impl Drop for Jason {
    fn drop(&mut self) {
        info!("Drop for Jason");
    }
}

pub struct ConnectionHandle;

impl ConnectionHandle {
    pub fn on_close(&self, _f: Arc<JavaCallback<()>>) -> Result<(), String> {
        Ok(())
    }

    pub fn get_remote_member_id(&self) -> Result<String, String> {
        Ok(String::from("remote_member_id"))
    }

    pub fn on_remote_track_added(
        &self,
        cb: Arc<JavaCallback<RemoteMediaTrack>>,
    ) -> Result<(), String> {
        info!(
            "ConnectionHandle::on_remote_track_added() {:?}",
            std::thread::current()
        );
        cb.accept(RemoteMediaTrack);
        Ok(())
    }

    pub fn on_quality_score_update(
        &self,
        cb: Arc<JavaCallback<u8>>,
    ) -> Result<(), String> {
        cb.accept(1);
        Ok(())
    }
}

pub struct RoomHandle;

impl RoomHandle {
    pub fn on_new_connection(
        &mut self,
        cb: Arc<JavaCallback<ConnectionHandle>>,
    ) -> Result<(), String> {
        cb.accept(ConnectionHandle);
        Ok(())
    }

    pub fn on_close(
        &mut self,
        _f: Arc<JavaCallback<RoomCloseReason>>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn on_local_track(
        &self,
        _f: Arc<JavaCallback<LocalMediaTrack>>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn on_failed_local_media(
        &self,
        _f: Arc<JavaCallback<JasonError>>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub fn on_connection_loss(
        &self,
        _f: Arc<JavaCallback<ReconnectHandle>>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub async fn join(&self, _token: String) -> Result<(), String> {
        Ok(())
    }

    pub async fn set_local_media_settings(
        &self,
        _settings: &MediaStreamSettings,
        _stop_first: bool,
        _rollback_on_fail: bool,
    ) -> Result<(), String> {
        Ok(())
    }

    pub async fn mute_audio(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn unmute_audio(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn mute_video(
        &self,
        _source_kind: Option<MediaSourceKind>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub async fn unmute_video(
        &self,
        _source_kind: Option<MediaSourceKind>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub async fn disable_audio(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn enable_audio(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn disable_video(
        &self,
        _source_kind: Option<MediaSourceKind>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub async fn enable_video(
        &self,
        _source_kind: Option<MediaSourceKind>,
    ) -> Result<(), String> {
        Ok(())
    }

    pub async fn disable_remote_audio(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn disable_remote_video(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn enable_remote_audio(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn enable_remote_video(&self) -> Result<(), String> {
        Ok(())
    }
}

pub struct MediaManagerHandle;

impl MediaManagerHandle {
    pub async fn enumerate_devices(
        &self,
    ) -> Result<Vec<InputDeviceInfo>, String> {
        Ok(Vec::new())
    }

    pub async fn init_local_tracks(
        &self,
        _caps: &MediaStreamSettings,
    ) -> Result<Vec<LocalMediaTrack>, String> {
        Ok(Vec::new())
    }
}

pub struct JasonError;

impl JasonError {
    pub fn name(&self) -> String {
        String::from("JasonError::name()")
    }

    pub fn message(&self) -> String {
        String::from("JasonError::message()")
    }

    pub fn trace(&self) -> String {
        String::from("JasonError::trace()")
    }

    // pub fn source(&self) -> Option<sys::Error>
}

pub struct InputDeviceInfo;

impl InputDeviceInfo {
    pub fn device_id(&self) -> String {
        String::from("InputDeviceInfo::device_id()")
    }

    pub fn kind(&self) -> MediaKind {
        MediaKind::Video
    }

    pub fn label(&self) -> String {
        String::from("InputDeviceInfo::label()")
    }

    pub fn group_id(&self) -> String {
        String::from("InputDeviceInfo::group_id()")
    }
}

pub struct MediaStreamSettings;

impl MediaStreamSettings {
    pub fn audio(&mut self, _constraints: AudioTrackConstraints) {}

    pub fn device_video(&mut self, _constraints: DeviceVideoTrackConstraints) {}

    pub fn display_video(
        &mut self,
        _constraints: DisplayVideoTrackConstraints,
    ) {
    }
}

pub struct DisplayVideoTrackConstraints;

pub struct AudioTrackConstraints;

impl AudioTrackConstraints {
    pub fn device_id(&mut self, _device_id: String) {}
}

pub struct DeviceVideoTrackConstraints;

impl DeviceVideoTrackConstraints {
    pub fn device_id(&mut self, _device_id: String) {}

    pub fn exact_facing_mode(&mut self, _facing_mode: FacingMode) {}

    pub fn ideal_facing_mode(&mut self, _facing_mode: FacingMode) {}

    pub fn exact_height(&mut self, _height: u32) {}

    pub fn ideal_height(&mut self, _height: u32) {}

    pub fn height_in_range(&mut self, _min: u32, _max: u32) {}

    pub fn exact_width(&mut self, _width: u32) {}

    pub fn ideal_width(&mut self, _width: u32) {}

    pub fn width_in_range(&mut self, _min: u32, _max: u32) {}
}

pub struct LocalMediaTrack;

impl LocalMediaTrack {
    // pub fn get_track(&self) -> sys::MediaStreamTrack {}
    pub fn kind(&self) -> MediaKind {
        MediaKind::Video
    }

    pub fn media_source_kind(&self) -> MediaSourceKind {
        MediaSourceKind::Display
    }
}

pub struct RemoteMediaTrack;

impl RemoteMediaTrack {
    // pub fn get_track(&self) -> sys::MediaStreamTrack {}
    pub fn enabled(&self) -> bool {
        true
    }

    pub fn on_enabled(&self, cb: Arc<JavaCallback<()>>) {
        cb.accept();
    }

    pub fn on_disabled(&self, _cb: Arc<JavaCallback<()>>) {}

    pub fn kind(&self) -> MediaKind {
        MediaKind::Video
    }

    pub fn media_source_kind(&self) -> MediaSourceKind {
        MediaSourceKind::Device
    }
}

pub struct RoomCloseReason;

impl RoomCloseReason {
    pub fn reason(&self) -> String {
        String::from("RoomCloseReason::reason")
    }

    pub fn is_closed_by_server(&self) -> bool {
        false
    }

    pub fn is_err(&self) -> bool {
        false
    }
}

pub struct ConstraintsUpdateException;

impl ConstraintsUpdateException {
    pub fn name(&self) -> String {
        String::from("ConstraintsUpdateException::name")
    }

    pub fn recover_reason(&self) -> Option<JasonError> {
        None
    }

    pub fn recover_fail_reasons(&self) -> Vec<JasonError> {
        vec![JasonError {}]
    }

    pub fn error(&self) -> Option<JasonError> {
        None
    }
}

pub struct ReconnectHandle;

impl ReconnectHandle {
    pub async fn reconnect_with_delay(
        &self,
        _delay_ms: u32,
    ) -> Result<(), String> {
        Ok(())
    }

    pub async fn reconnect_with_backoff(
        &self,
        _starting_delay_ms: u32,
        _multiplier: f32,
        _max_delay: u32,
    ) -> Result<(), String> {
        Ok(())
    }
}
