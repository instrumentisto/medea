use dart_sys::{
    Dart_DeletePersistentHandle, Dart_Handle, Dart_HandleFromPersistent,
    Dart_NewPersistentHandle, Dart_PersistentHandle,
};

#[derive(Clone, Debug, PartialEq)]
pub struct DartHandle(Dart_PersistentHandle);

impl From<Dart_Handle> for DartHandle {
    fn from(handle: Dart_Handle) -> Self {
        Self::new(handle)
    }
}

impl DartHandle {
    pub fn new(handle: Dart_Handle) -> Self {
        Self(unsafe { Dart_NewPersistentHandle(handle) })
    }

    pub fn get(&self) -> Dart_Handle {
        unsafe { Dart_HandleFromPersistent(self.0) }
    }
}

impl Drop for DartHandle {
    fn drop(&mut self) {
        unsafe {
            Dart_DeletePersistentHandle(self.0);
        }
    }
}
