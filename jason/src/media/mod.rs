//! Adapters to [Media Capture and Streams API][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams

mod constraints;
mod device_info;
mod manager;
mod stream;

#[doc(inline)]
pub use self::{
    constraints::{
        AudioTrackConstraints, DeviceVideoTrackConstraints,
        DisplayVideoTrackConstraints, FacingMode, LocalStreamConstraints,
        MediaStreamSettings, MediaStreamTrackConstraints,
        MultiSourceMediaStreamConstraints, RecvConstraints, TrackConstraints,
        VideoTrackConstraints,
    },
    device_info::InputDeviceInfo,
    manager::{MediaManager, MediaManagerError, MediaManagerHandle},
    stream::{
        MediaStream, MediaStreamTrack, MediaStreamTrackHandle, TrackKind,
    },
};
