use std::rc::Rc;

use dart_sys::{Dart_Handle, Dart_PersistentHandle};

use crate::platform::dart::utils::dart_api::{
    Dart_DeletePersistentHandle_DL_Trampolined,
    Dart_HandleFromPersistent_DL_Trampolined,
    Dart_NewPersistentHandle_DL_Trampolined,
};

#[derive(Debug, PartialEq)]
struct Inner(Dart_PersistentHandle);

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            Dart_DeletePersistentHandle_DL_Trampolined(self.0);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DartHandle(Rc<Inner>);

impl From<Dart_Handle> for DartHandle {
    fn from(handle: Dart_Handle) -> Self {
        Self::new(handle)
    }
}

impl DartHandle {
    pub fn new(handle: Dart_Handle) -> Self {
        Self(Rc::new(Inner(unsafe {
            Dart_NewPersistentHandle_DL_Trampolined(handle)
        })))
    }

    pub fn get(&self) -> Dart_Handle {
        unsafe { Dart_HandleFromPersistent_DL_Trampolined(self.0 .0) }
    }
}
