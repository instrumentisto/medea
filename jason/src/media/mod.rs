//! Adapters to [Media Capture and Streams API][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams

mod constraints;
mod device_info;
mod manager;
mod stream;
mod stream_request;
mod track;

#[doc(inline)]
pub use self::{
    constraints::{
        AudioTrackConstraints, MediaStreamConstraints, VideoTrackConstraints,
    },
    device_info::InputDeviceInfo,
    manager::{MediaManager, MediaManagerHandle},
    stream::{MediaStream, MediaStreamHandle},
    stream_request::{SimpleStreamRequest, StreamRequest},
    track::MediaTrack,
};
