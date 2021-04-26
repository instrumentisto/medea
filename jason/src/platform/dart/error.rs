use derive_more::Display;

use crate::{
    platform::dart::utils::handle::DartHandle, utils::dart::from_dart_string,
};
use dart_sys::{Dart_Handle, _Dart_Handle};

/// Wrapper for Dart value which returned from Dart side as error.
#[derive(Clone, Debug, Display, PartialEq)]
#[display(fmt = "{}: {}", name, message)]
pub struct Error {
    /// Name of JS error.
    pub name: String,

    /// Message of JS error.
    pub message: String,

    pub sys_cause: Option<DartHandle>,
}

impl From<DartError> for Error {
    fn from(e: DartError) -> Self {
        Self {
            name: e.name(),
            message: e.message(),
            sys_cause: Some(e.0),
        }
    }
}

type NameFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut NAME_FUNCTION: Option<NameFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_DartError__name(f: NameFunction) {
    NAME_FUNCTION = Some(f)
}

type MessageFunction = extern "C" fn(Dart_Handle) -> *const libc::c_char;
static mut MESSAGE_FUNCTION: Option<MessageFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_DartError__message(f: MessageFunction) {
    MESSAGE_FUNCTION = Some(f);
}

#[derive(Clone, Debug, PartialEq)]
pub struct DartError(DartHandle);

impl DartError {
    pub fn name(&self) -> String {
        unsafe { from_dart_string(NAME_FUNCTION.unwrap()(self.0.get())) }
    }

    pub fn message(&self) -> String {
        unsafe { from_dart_string(MESSAGE_FUNCTION.unwrap()(self.0.get())) }
    }
}

impl From<Dart_Handle> for DartError {
    fn from(from: Dart_Handle) -> Self {
        Self(DartHandle::new(from))
    }
}
