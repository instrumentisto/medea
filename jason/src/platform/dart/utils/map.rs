use dart_sys::Dart_Handle;

use crate::{
    platform::dart::utils::handle::DartHandle, utils::dart::into_dart_string,
};

pub struct DartMap(DartHandle);

impl From<DartMap> for Value {
    fn from(from: DartMap) -> Self {
        Self::Map(from)
    }
}

impl From<String> for Value {
    fn from(from: String) -> Self {
        Self::String(from)
    }
}

impl From<i32> for Value {
    fn from(from: i32) -> Self {
        Self::Int(from)
    }
}

impl Into<Dart_Handle> for DartMap {
    fn into(self) -> Dart_Handle {
        self.0.get()
    }
}

type NewFunction = extern "C" fn() -> Dart_Handle;
static mut NEW_FUNCTION: Option<NewFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_DartMap__new(f: NewFunction) {
    NEW_FUNCTION = Some(f);
}

type SetFunction = extern "C" fn(Dart_Handle, *const libc::c_char, Dart_Handle);
static mut SET_FUNCTION: Option<SetFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_DartMap__set(f: SetFunction) {
    SET_FUNCTION = Some(f);
}

type RemoveFunction = extern "C" fn(Dart_Handle, *const libc::c_char);
static mut REMOVE_FUNCTION: Option<RemoveFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_DartMap__remove(f: RemoveFunction) {
    REMOVE_FUNCTION = Some(f);
}

impl DartMap {
    pub fn new() -> Self {
        Self(DartHandle::new(unsafe { NEW_FUNCTION.unwrap()() }))
    }

    pub fn set(&self, key: String, value: Value) {
        unsafe {
            SET_FUNCTION.unwrap()(
                self.0.get(),
                into_dart_string(key),
                value.into(),
            )
        }
    }

    pub fn remove(&self, key: String) {
        unsafe { REMOVE_FUNCTION.unwrap()(self.0.get(), into_dart_string(key)) }
    }
}

type NewStringFunction = extern "C" fn(*const libc::c_char) -> Dart_Handle;
static mut NEW_STRING_FUNCTION: Option<NewStringFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_String__new(f: NewStringFunction) {
    NEW_STRING_FUNCTION = Some(f);
}

type NewIntFunction = extern "C" fn(i32) -> Dart_Handle;
static mut NEW_INT_FUNCTION: Option<NewIntFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_Int__new(f: NewIntFunction) {
    NEW_INT_FUNCTION = Some(f);
}

pub enum Value {
    Map(DartMap),
    String(String),
    Int(i32),
}

impl Into<Dart_Handle> for Value {
    fn into(self) -> Dart_Handle {
        match self {
            Self::Map(h) => h.0.get(),
            Self::String(s) => unsafe {
                NEW_STRING_FUNCTION.unwrap()(into_dart_string(s))
            },
            Self::Int(i) => unsafe { NEW_INT_FUNCTION.unwrap()(i) },
        }
    }
}