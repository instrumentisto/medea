use crate::utils::{window, WasmErr};
use futures::future::{self, Either};
use futures::Future;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::MediaStream as BackingMediaStream;

#[derive(Default)]
pub struct MediaManager(Rc<RefCell<InnerMediaManager>>);

#[derive(Default)]
struct InnerMediaManager {
    streams: Vec<Rc<MediaStream>>,
}

impl MediaManager {
    pub fn get_stream(
        &self,
        caps: MediaCaps,
    ) -> impl Future<Item = Rc<MediaStream>, Error = WasmErr> {
        // TODO: lookup stream by its caps, return its copy

        let stream = match self.inner_get_stream(&caps) {
            Ok(promise) => JsFuture::from(promise),
            Err(err) => return Either::A(future::err(err)),
        };

        let inner = Rc::clone(&self.0);
        let fut = stream
            .and_then(move |stream| {
                let stream = MediaStream::new(BackingMediaStream::from(stream));
                inner.borrow_mut().streams.push(Rc::clone(&stream));
                Ok(stream)
            })
            .map_err(WasmErr::from);

        Either::B(fut)
    }

    fn inner_get_stream(
        &self,
        caps: &MediaCaps,
    ) -> Result<js_sys::Promise, WasmErr> {
        window()
            .navigator()
            .media_devices()?
            .get_user_media_with_constraints(&caps.into())
            .map_err(WasmErr::from)
    }
}

pub struct MediaCaps {
    audio: bool,
    video: bool,
}

impl MediaCaps {
    pub fn new(audio: bool, video: bool) -> Result<MediaCaps, WasmErr> {
        if !audio && !video {
            return Err(WasmErr::from_str(
                "MediaCaps should have video, audio, or both",
            ));
        }
        Ok(MediaCaps { audio, video })
    }
}

impl From<&MediaCaps> for web_sys::MediaStreamConstraints {
    fn from(caps: &MediaCaps) -> Self {
        let mut constraints = web_sys::MediaStreamConstraints::new();
        if caps.audio {
            constraints.audio(&JsValue::from_bool(true));
        }
        if caps.video {
            constraints.video(&JsValue::from_bool(true));
        }
        constraints
    }
}

struct InnerStream {
    stream: BackingMediaStream,
}

impl From<BackingMediaStream> for InnerStream {
    fn from(media_stream: BackingMediaStream) -> Self {
        InnerStream {
            stream: media_stream,
        }
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct MediaStream(Rc<InnerStream>);

impl MediaStream {
    pub fn new(stream: BackingMediaStream) -> Rc<Self> {
        Rc::new(Self(Rc::new(InnerStream::from(stream))))
    }

    pub fn new_handle(&self) -> MediaStreamHandle {
        MediaStreamHandle(Rc::downgrade(&self.0))
    }

    pub fn get_media_stream(&self) -> BackingMediaStream {
        self.0.stream.clone()
    }
}

#[wasm_bindgen]
pub struct MediaStreamHandle(Weak<InnerStream>);

#[wasm_bindgen]
impl MediaStreamHandle {
    pub fn get_media_stream(&self) -> Result<BackingMediaStream, JsValue> {
        match self.0.upgrade() {
            Some(inner) => Ok(inner.stream.clone()),
            None => Err(WasmErr::from_str("Detached state").into()),
        }
    }
}
