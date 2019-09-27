//! Acquiring and storing [MediaStream]s.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use wasm_bindgen_futures::JsFuture;
use web_sys::MediaStream as SysMediaStream;

use crate::utils::{window, Callback2, WasmErr};

use super::{
    MediaStream, MediaStreamHandle, SimpleStreamRequest, StreamRequest,
};

/// Manager that is responsible for [`MediaStream`] acquisition and storing.
#[allow(clippy::module_name_repetitions)]
#[derive(Default)]
pub struct MediaManager(Rc<RefCell<InnerMediaManager>>);

/// Actual data of [`MediaManager`].
#[derive(Default)]
struct InnerMediaManager {
    /// Obtained streams.
    streams: Vec<Rc<MediaStream>>,

    /// Callback to be invoked when new [`MediaStream`] is acquired providing
    /// its handle.
    // TODO: will be extended with some metadata that would allow client to
    //       understand purpose of obtaining this stream.
    on_local_stream: Callback2<MediaStreamHandle, WasmErr>,
}

impl MediaManager {
    /// Obtain [`MediaStream`] basing on a provided [`StreamRequest`].
    /// Acquired streams are cached and cloning existing stream is preferable
    /// over obtaining new ones.
    ///
    /// `on_local_stream` callback will be invoked each time this function
    /// succeeds.
    // TODO: lookup stream by caps, and return its copy if found
    pub async fn get_stream(
        &self,
        caps: StreamRequest,
    ) -> Result<Rc<MediaStream>, WasmErr> {
        let request = SimpleStreamRequest::try_from(caps)?;

        let constraints = web_sys::MediaStreamConstraints::from(&request);

        let media_devices = window()
            .navigator()
            .media_devices()
            .map_err(WasmErr::from)?;

        let get_user_media = media_devices
            .get_user_media_with_constraints(&constraints)
            .map_err(WasmErr::from)?;
        let stream = JsFuture::from(get_user_media).await?;

        match request.parse_stream(&SysMediaStream::from(stream)) {
            Ok(stream) => {
                let stream = Rc::new(stream);
                self.0.borrow_mut().streams.push(Rc::clone(&stream));
                self.0.borrow().on_local_stream.call1(stream.new_handle());
                Ok(stream)
            }
            Err(err) => {
                self.0.borrow().on_local_stream.call2(err.clone());
                Err(err)
            }
        }
    }

    /// Sets `on_local_stream` callback that will be invoked when
    /// [`MediaManager`] obtains [`MediaStream`].
    #[inline]
    pub fn set_on_local_stream(&self, f: js_sys::Function) {
        self.0.borrow_mut().on_local_stream.set_func(f);
    }
}
