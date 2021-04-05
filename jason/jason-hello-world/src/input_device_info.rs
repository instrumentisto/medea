pub struct InputDeviceInfo {
    pub foo: u64,
    pub bar: u32,
}

impl InputDeviceInfo {
    pub fn kind(&self) -> DeviceKind {
        DeviceKind::Foo
    }

    pub fn label(&self) -> String {
        "foobar".to_string()
    }

    pub fn group_id(&self) -> String {
        "foobar".to_string()
    }
}

pub enum DeviceKind {
    Foo,
}

impl Into<u8> for DeviceKind {
    fn into(self) -> u8 {
        0
    }
}

impl InputDeviceInfo {
    pub fn device_id(&self) -> String {
        format!("foo {} - bar {}", self.foo, self.bar)
    }
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__device_id(
    this: *mut InputDeviceInfo,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    super::into_dart_string(this.device_id())
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__kind(
    this: *mut InputDeviceInfo,
) -> u8 {
    let this = Box::from_raw(this);
    this.kind().into()
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo__label<'a>(
    this: *mut InputDeviceInfo,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    super::into_dart_string(this.label())
}

#[no_mangle]
pub unsafe extern "C" fn InputDeviceInfo_nativeGroupId<'a>(
    this: *mut InputDeviceInfo,
) -> *const libc::c_char {
    let this = Box::from_raw(this);
    super::into_dart_string(this.group_id())
}
