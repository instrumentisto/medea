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
