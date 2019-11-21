use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use anyhow::Result;
use web_sys::MediaStream as SysMediaStream;

use crate::{
    media::MediaManager,
    peer::{
        MediaSource, MediaStream, MediaStreamHandle, SimpleStreamRequest,
        StreamRequest,
    },
    utils::{Callback, PinFuture},
};

/// Storage the local [MediaStream][1] for [`Room`] and callbacks for success
/// and fail get local [`MediaStream`][1] from [`MediaManager`].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
pub struct RoomStorage {
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
    on_fail: Rc<Callback<js_sys::Error>>,
}

impl RoomStorage {
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

impl MediaSource for RoomStorage {
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
    ) -> PinFuture<Result<MediaStream>> {
        let local_stream = Rc::clone(&self.local_stream);
        let media_manager = Rc::clone(&self.media_manager);
        let success = Rc::clone(&self.on_success);
        let fail = Rc::clone(&self.on_fail);
        Box::pin(async move {
            match async move {
                let caps = SimpleStreamRequest::try_from(request)?;
                if let Some(stream) = local_stream.borrow().as_ref() {
                    Ok((caps.parse_stream(stream)?, false))
                } else {
                    let (stream, is_new) =
                        media_manager.get_stream(&caps).await?;
                    Ok((caps.parse_stream(&stream)?, is_new))
                }
            }
            .await
            {
                Ok((stream, is_new)) => {
                    if is_new {
                        success.call(stream.new_handle());
                    }
                    Ok(stream)
                }
                Err(err) => {
                    fail.call(js_sys::Error::new(&format!("{}", err)));
                    Err(err)
                }
            }
        })
    }
}
