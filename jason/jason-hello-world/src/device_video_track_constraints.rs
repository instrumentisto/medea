use std::convert::TryFrom;

pub struct DeviceVideoTrackConstraints;

impl DeviceVideoTrackConstraints {
    pub fn device_id(&mut self, id: String) {}

    pub fn exact_facing_mode(&mut self, facing_mode: FacingMode) {}

    pub fn ideal_facing_mode(&mut self, facing_mode: FacingMode) {}

    pub fn exact_height(&mut self, height: u32) {}

    pub fn ideal_height(&mut self, height: u32) {}

    pub fn height_in_range(&mut self, min: u32, max: u32) {}

    pub fn exact_width(&mut self, width: u32) {}

    pub fn ideal_width(&mut self, width: u32) {}

    pub fn width_in_range(&mut self, min: u32, max: u32) {}
}

pub enum FacingMode {}

impl TryFrom<u8> for FacingMode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        todo!()
    }
}
