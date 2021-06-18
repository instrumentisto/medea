//! Exception returned from [`RoomHandle::set_local_media_settings()`][1].
//!
//! [1]: crate::api::RoomHandle::set_local_media_settings

use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::room;

use super::JasonError;

/// Exception returned from [`RoomHandle::set_local_media_settings()`][1].
///
/// [1]: crate::api::RoomHandle::set_local_media_settings
#[wasm_bindgen]
#[derive(Debug, From)]
#[from(forward)]
pub struct ConstraintsUpdateException(room::ConstraintsUpdateException);

#[wasm_bindgen]
impl ConstraintsUpdateException {
    /// Returns name of this [`ConstraintsUpdateException`].
    #[must_use]
    pub fn name(&self) -> String {
        self.0.name()
    }

    /// Returns [`JasonError`] if this [`ConstraintsUpdateException`] represents
    /// a `RecoveredException` or a `RecoverFailedException`.
    ///
    /// Returns `undefined` otherwise.
    pub fn recover_reason(&self) -> Option<JasonError> {
        self.0.recover_reason().map(Into::into)
    }

    /// Returns [`js_sys::Array`] with the [`JasonError`]s if this
    /// [`ConstraintsUpdateException`] represents a `RecoverFailedException`.
    #[must_use]
    pub fn recover_fail_reasons(&self) -> JsValue {
        self.0
            .recover_fail_reasons()
            .into_iter()
            .map(JasonError::from)
            .map(JsValue::from)
            .collect::<js_sys::Array>()
            .into()
    }

    /// Returns [`JasonError`] if this [`ConstraintsUpdateException`] represents
    /// an `ErroredException`.
    ///
    /// Returns `undefined` otherwise.
    pub fn error(&self) -> Option<JasonError> {
        self.0.error().map(Into::into)
    }
}
