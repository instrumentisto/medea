//! Implementations and definitions of the errors which can be returned from the
//! API functions.

use derive_more::{From, Into};
use wasm_bindgen::{
    convert::{FromWasmAbi, IntoWasmAbi},
    describe::WasmDescribe,
    prelude::*,
};

use crate::api::errors::{
    EnumerateDevicesException, FormatException, InternalException,
    LocalMediaInitException, MediaSettingsUpdateException,
    MediaStateTransitionException, RpcClientException, StateError,
};

/// Wrapper around [`JsValue`] which represents JS error.
#[derive(Into, From)]
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

impl FromWasmAbi for Error {
    type Abi = <JsValue as FromWasmAbi>::Abi;

    unsafe fn from_abi(js: Self::Abi) -> Self {
        Self(FromWasmAbi::from_abi(js))
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