use core::ffi::c_void;

/// Raw C userdata type
pub type UserData = *mut c_void;

/// C userdata type
#[repr(transparent)]
pub struct WrappedUserData(UserData);

unsafe impl Send for WrappedUserData {}
unsafe impl Sync for WrappedUserData {}

impl From<UserData> for WrappedUserData {
    fn from(raw: UserData) -> Self {
        Self(raw)
    }
}

impl Into<UserData> for WrappedUserData {
    fn into(self) -> UserData {
        self.0
    }
}

impl core::ops::Deref for WrappedUserData {
    type Target = UserData;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for WrappedUserData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
