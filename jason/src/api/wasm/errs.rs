//! asdasdasd

use derive_more::Into;
use wasm_bindgen::prelude::*;

use crate::api::errors::{
    EnumerateDevicesException, FormatException, InternalException,
    LocalMediaInitException, MediaSettingsUpdateException,
    MediaStateTransitionException, RpcClientException, StateError,
};
use wasm_bindgen::{convert::IntoWasmAbi, describe::WasmDescribe};

/// asdasd
#[derive(Into)]
pub struct Error(JsValue);

// So we could use Error as return type in exported functions.
impl WasmDescribe for Error {
    fn describe() {
        JsValue::describe()
    }
}

// So we could use Error as return type in exported functions.
impl IntoWasmAbi for Error {
    type Abi = <JsValue as IntoWasmAbi>::Abi;

    #[inline]
    fn into_abi(self) -> Self::Abi {
        self.0.into_abi()
    }
}

impl From<StateError> for Error {
    fn from(err: StateError) -> Self {
        Error(JsValue::from(err))
    }
}

impl From<EnumerateDevicesException> for Error {
    fn from(err: EnumerateDevicesException) -> Self {
        Error(JsValue::from(err))
    }
}

impl From<LocalMediaInitException> for Error {
    fn from(err: LocalMediaInitException) -> Self {
        Error(JsValue::from(err))
    }
}

impl From<RpcClientException> for Error {
    fn from(err: RpcClientException) -> Self {
        Error(JsValue::from(err))
    }
}

impl From<InternalException> for Error {
    fn from(err: InternalException) -> Self {
        Error(JsValue::from(err))
    }
}

impl From<FormatException> for Error {
    fn from(err: FormatException) -> Self {
        Error(JsValue::from(err))
    }
}

impl From<MediaStateTransitionException> for Error {
    fn from(err: MediaStateTransitionException) -> Self {
        Error(JsValue::from(err))
    }
}

impl From<MediaSettingsUpdateException> for Error {
    fn from(err: MediaSettingsUpdateException) -> Self {
        Error(JsValue::from(err))
    }
}
