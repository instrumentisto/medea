//! Adapters to [`Media Capture and Streams API`][1].
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams

mod manager;
mod stream;
mod stream_request;
mod track;

#[doc(inline)]
pub use self::{
    manager::MediaManager,
    stream::{MediaStream, MediaStreamHandle},
    stream_request::{SimpleStreamRequest, StreamRequest},
    track::MediaTrack,
};
