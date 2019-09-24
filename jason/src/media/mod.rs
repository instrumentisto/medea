//! Adapters to [`Media Capture and Streams API`][1].
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams

mod device_info;
mod manager;
mod stream;
mod stream_request;
mod track;

#[doc(inline)]
pub use self::{
    device_info::InputDeviceInfo,
    manager::{MediaManager, MediaManagerHandle},
    stream::{MediaStream, MediaStreamHandle},
    stream_request::{
        AudioTrackConstraints, MediaStreamConstraints, SimpleStreamRequest,
        StreamRequest, VideoTrackConstraints,
    },
    track::MediaTrack,
};
