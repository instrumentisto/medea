//! Adapters to [Media Capture and Streams API][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams

mod constraints;
mod device_info;
mod manager;

#[doc(inline)]
pub use self::{
    constraints::{
        AudioTrackConstraints, MediaStreamConstraints, TrackConstraints,
        VideoTrackConstraints,
    },
    device_info::InputDeviceInfo,
    manager::{MediaManager, MediaManagerHandle},
};
