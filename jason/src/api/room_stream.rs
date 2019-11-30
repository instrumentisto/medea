use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use derive_more::{Display, From};
use futures::future::LocalBoxFuture;
use tracerr::Traced;
use web_sys::MediaStream as SysMediaStream;

use crate::{
    media::{MediaManager, MediaManagerError},
    peer::{
        MediaSource, MediaStream, MediaStreamHandle, SimpleStreamRequest,
        StreamRequest, StreamRequestError,
    },
    utils::{Callback, JasonError, JsCaused, JsError},
};

/// Errors that may occur in process of receiving [`MediaStream`].
#[derive(Debug, Display, From, JsCaused)]
pub enum Error {
    /// Failed to get local stream from [`MediaManager`].
    #[display(fmt = "Failed to get local stream: {}", _0)]
    CouldNotGetLocalMedia(#[js(cause)] MediaManagerError),

    /// Errors that may occur when validating [`StreamRequest`] or
    /// parsing [`MediaStream`][1].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    #[display(fmt = "Invalid local stream: {}", _0)]
    InvalidLocalStream(#[js(cause)] StreamRequestError),
}

/// Storage the local [MediaStream][1] for [`Room`] and callbacks for success
/// and fail get [`MediaStream`].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
pub struct RoomStream {
    /// Local [`MediaStream`][1] injected into this [`Room`].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    local_stream: Rc<RefCell<Option<SysMediaStream>>>,

    /// [`MediaManager`] that will be used to acquire local
    /// [`MediaStream`][1]s.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    media_manager: Rc<MediaManager>,

    /// Callback to be invoked when new [`MediaStream`][1] is acquired.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    // TODO: will be extended with some metadata that would allow client to
    //       understand purpose of obtaining this stream.
    on_success: Rc<Callback<MediaStreamHandle>>,

    /// Callback to be invoked when fail obtain [`MediaStream`][1] from
    /// [`MediaManager`] or cannot parse its into [`MediaStream`].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    on_fail: Rc<Callback<JasonError>>,
}

impl RoomStream {
    /// Creates new [`RoomStorage`].
    pub fn new(media_manager: Rc<MediaManager>) -> Self {
        Self {
            local_stream: Rc::new(RefCell::new(None)),
            media_manager,
            on_success: Rc::new(Callback::default()),
            on_fail: Rc::new(Callback::default()),
        }
    }

    /// Store local [`MediaStream`][1].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    pub fn store_local_stream(&self, stream: SysMediaStream) {
        self.local_stream.borrow_mut().replace(stream);
    }

    /// Set callback to receive successfully [`MediaStream`].
    ///
    /// NOTE: Callback to invoke only if [`MediaStream`] acquired from NEW
    /// [`MediaStream`][1].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    pub fn on_success(&self, f: js_sys::Function) {
        self.on_success.set_func(f);
    }

    /// Set callback for fail receipt [`MediaStream`].
    pub fn on_fail(&self, f: js_sys::Function) {
        self.on_fail.set_func(f);
    }

    /// Indicates if `on_fail` callback is set.
    pub fn is_set_on_fail(&self) -> bool {
        self.on_fail.is_set()
    }
}

impl MediaSource for RoomStream {
    type Error = Error;

    /// Returns the stored local media stream if exists or retrieve new from
    /// [`MediaManager`].
    ///
    /// Invokes the callback `on_local_stream` on success retrieving new stream
    /// from media manager. Invokes `on_failed_local_stream` on:
    ///    - fail retrieving stream from media manager;
    ///    - fail convert local stream into [`MediaStream`].
    fn get_media_stream(
        &self,
        request: StreamRequest,
    ) -> LocalBoxFuture<Result<MediaStream, Traced<Self::Error>>> {
        let local_stream = Rc::clone(&self.local_stream);
        let media_manager = Rc::clone(&self.media_manager);
        let success = Rc::clone(&self.on_success);
        let fail = Rc::clone(&self.on_fail);
        Box::pin(async move {
            async move {
                let caps = SimpleStreamRequest::try_from(request)
                    .map_err(tracerr::from_and_wrap!())?;
                if let Some(stream) = local_stream.borrow().as_ref() {
                    Ok((
                        caps.parse_stream(stream)
                            .map_err(tracerr::map_from_and_wrap!())?,
                        false,
                    ))
                } else {
                    let (stream, is_new) = media_manager
                        .get_stream(&caps)
                        .await
                        .map_err(tracerr::map_from_and_wrap!())?;
                    Ok((
                        caps.parse_stream(&stream)
                            .map_err(tracerr::map_from_and_wrap!())?,
                        is_new,
                    ))
                }
            }
            .await
            .map(|(stream, is_new)| {
                if is_new {
                    success.call(stream.new_handle());
                }
                stream
            })
            .map_err(|e: Traced<Error>| {
                fail.call(JasonError::new(
                    e.as_ref().name(),
                    e.as_ref(),
                    e.trace().clone(),
                    e.as_ref().js_cause(),
                ));
                e
            })
        })
    }
}
