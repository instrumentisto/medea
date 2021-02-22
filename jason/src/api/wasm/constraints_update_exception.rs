use derive_more::From;
use wasm_bindgen::prelude::*;

use crate::core;

/// JS exception for the [`RoomHandle::set_local_media_settings`].
#[wasm_bindgen]
#[derive(Debug, From)]
#[from(forward)]
pub struct ConstraintsUpdateException(core::ConstraintsUpdateException);

#[wasm_bindgen]
impl ConstraintsUpdateException {
    /// Returns name of this [`ConstraintsUpdateException`].
    pub fn name(&self) -> String {
        self.0.name()
    }

    /// Returns [`JasonError`] if this [`ConstraintsUpdateException`] represents
    /// `RecoveredException` or `RecoverFailedException`.
    ///
    /// Returns `undefined` otherwise.
    pub fn recover_reason(&self) -> JsValue {
        self.0.recover_reason()
    }

    /// Returns [`js_sys::Array`] with the [`JasonError`]s if this
    /// [`ConstraintsUpdateException`] represents `RecoverFailedException`.
    ///
    /// Returns `undefined` otherwise.
    pub fn recover_fail_reasons(&self) -> JsValue {
        self.0.recover_fail_reasons()
    }

    /// Returns [`JasonError`] if this [`ConstraintsUpdateException`] represents
    /// `ErroredException`.
    ///
    /// Returns `undefined` otherwise.
    pub fn error(&self) -> JsValue {
        self.0.error()
    }
}
