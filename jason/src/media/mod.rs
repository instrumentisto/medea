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
    device_info::MediaDeviceInfo,
    manager::MediaManager,
    stream::{MediaStream, MediaStreamHandle},
    stream_request::{SimpleStreamRequest, StreamRequest},
    track::{Id as TrackId, MediaTrack},
};
