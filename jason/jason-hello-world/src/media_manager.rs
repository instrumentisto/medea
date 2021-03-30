use crate::input_device_info::InputDeviceInfo;

pub struct MediaManager;

impl MediaManager {
    pub fn enumerate_devices(&self) -> Vec<InputDeviceInfo> {
        vec![InputDeviceInfo]
    }
}

pub unsafe extern "C" fn MediaManager__enumerate_devices(
    this: *mut MediaManager,
) -> *const InputDeviceInfo {
    let this = Box::from_raw(this);
    this.enumerate_devices().as_ptr()
}
