//! Exception returned from [`RoomHandle::set_local_media_settings()`][1].
//!
//! [1]: crate::api::RoomHandle::set_local_media_settings

use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::{api::wasm::Error, room};

/// Exception returned from [`RoomHandle::set_local_media_settings()`][1].
///
/// [1]: crate::api::RoomHandle::set_local_media_settings
#[wasm_bindgen]
#[derive(Debug, From)]
#[from(forward)]
pub struct ConstraintsUpdateException(room::ConstraintsUpdateError);

#[wasm_bindgen]
impl ConstraintsUpdateException {
    /// Returns a name of this [`ConstraintsUpdateError`].
    #[must_use]
    pub fn name(&self) -> String {
        self.0.name()
    }

    /// Returns a [`ChangeMediaStateError`] if this [`ConstraintsUpdateError`]
    /// represents a `RecoveredException` or a `RecoverFailedException`.
    #[must_use]
    pub fn recover_reason(&self) -> Error {
        self.0
            .recover_reason()
            .map(Into::into)
            .unwrap_or_else(|| JsValue::null().into())
    }

    /// Returns a list of [`ChangeMediaStateError`]s due to which a recovery
    /// has failed.
    #[must_use]
    pub fn recover_fail_reasons(&self) -> Vec<JsValue> {
        self.0
            .recover_fail_reasons()
            .into_iter()
            .map(|e| Error::from(e).into())
            .collect()
    }

    /// Returns a [`ChangeMediaStateError`] if this [`ConstraintsUpdateError`]
    /// represents an `ErroredException`.
    #[must_use]
    pub fn error(&self) -> Error {
        self.0
            .error()
            .map(Into::into)
            .unwrap_or_else(|| JsValue::null().into())
    }
}
