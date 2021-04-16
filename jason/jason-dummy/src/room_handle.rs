pub struct RoomHandle;

impl RoomHandle {
    // pub async fn join(&self, token: String) -> Result<(), JasonError>
    // pub fn on_new_connection(&self, f: Callback<ConnectionHandle>) ->
    // Result<(), JasonError> {} pub fn on_close(&mut self, f:
    // Callback<RoomCloseReason>) -> Result<(), JasonError>
    // pub fn on_local_track(&self, f: Callback<LocalMediaTrack>) -> Result<(),
    // JasonError> pub fn on_failed_local_media(&self, f:
    // Callback<JasonError>) -> Result<(), JasonError>
    // pub fn on_connection_loss(&self, f: Callback<ReconnectHandle>) ->
    // Result<(), JasonError> pub async fn set_local_media_settings(&self,
    // settings: &MediaStreamSettings, stop_first: bool, rollback_on_fail: bool)
    // -> Result<(), ConstraintsUpdateException> pub async fn
    // mute_audio(&self) -> Result<(), JasonError> pub async fn
    // unmute_audio(&self) -> Result<(), JasonError> pub async fn
    // mute_video(&self, source_kind: Option<MediaSourceKind>) -> Result<(),
    // JasonError> pub async fn unmute_video(&self, source_kind:
    // Option<MediaSourceKind>) -> Result<(), JasonError> pub async fn
    // disable_audio(&self) -> Result<(), JasonError> pub async fn
    // enable_audio(&self) -> Result<(), JasonError> pub async fn
    // disable_video(&self, source_kind: Option<MediaSourceKind>) -> Result<(),
    // JasonError> pub async fn enable_video(&self,source_kind:
    // Option<MediaSourceKind>) -> Result<(), JasonError> pub async fn
    // disable_remote_audio(&self) -> Result<(), JasonError> pub async fn
    // disable_remote_video(&self) -> Result<(), JasonError> pub async fn
    // enable_remote_audio(&self) -> Result<(), JasonError> pub async fn
    // enable_remote_video(&self) -> Result<(), JasonError>
}

#[no_mangle]
pub unsafe extern "C" fn RoomHandle__free(this: *mut RoomHandle) {
    Box::from_raw(this);
}
