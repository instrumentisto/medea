//! Adapters to [Media Capture and Streams API][1].
//!
//! [1]: https://w3.org/TR/mediacapture-streams

mod constraints;
mod device_info;
mod manager;

use futures::future::LocalBoxFuture;
use tracerr::Traced;

use crate::peer::{MediaStream, StreamRequest};

#[doc(inline)]
pub use self::{
    constraints::{
        AudioTrackConstraints, MediaStreamConstraints, TrackConstraints,
        VideoTrackConstraints,
    },
    device_info::InputDeviceInfo,
    manager::{MediaManager, MediaManagerError, MediaManagerHandle},
};

/// Source for acquire [`MediaStream`] by [`StreamRequest`].
#[allow(clippy::module_name_repetitions)]
pub trait MediaSource {
    /// Error that is returned if cannot receive the [`MediaStream`].
    type Error;

    /// Returns [`MediaStream`] by [`StreamRequest`].
    fn get_media_stream(
        &self,
        request: StreamRequest,
    ) -> LocalBoxFuture<Result<MediaStream, Traced<Self::Error>>>;
}
