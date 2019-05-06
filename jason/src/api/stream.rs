use crate::utils::{window, WasmErr};
use futures::future::{self, Either};
use futures::Future;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::rc::Rc;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{MediaStream, MediaStreamTrack};

#[derive(Default)]
pub struct MediaManager(Rc<RefCell<InnerMediaManager>>);

#[derive(Default)]
struct InnerMediaManager {
    streams: Vec<Rc<Stream>>,
}

impl MediaManager {

    pub fn get_stream(
        &mut self,
        caps: MediaCaps,
    ) -> impl Future<Item = Rc<Stream>, Error = WasmErr> {
        // TODO: lookup stream by its caps, return its copy

        let stream = match self.inner_get_stream(&caps) {
            Ok(promise) => JsFuture::from(promise),
            Err(err) => return Either::A(future::err(err)),
        };

        let inner = Rc::clone(&self.0);
        let fut = stream
            .and_then(move |stream| {
                let stream = Stream::new(MediaStream::from(stream))?;

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
    fn new(audio: bool, video: bool) -> Result<MediaCaps, WasmErr> {
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
    audio: Option<MediaStreamTrack>,
    video: Option<MediaStreamTrack>,
}

struct Stream(Rc<RefCell<Option<InnerStream>>>);

impl Stream {
    pub fn new(stream: MediaStream) -> Result<Rc<Self>, WasmErr> {
        Ok(Rc::new(Self(Rc::new(RefCell::new(Some(
            InnerStream::try_from(stream)?,
        ))))))
    }

    pub fn new_handle(&self) -> StreamHandle {
        StreamHandle(Rc::clone(&self.0))
    }
}

#[wasm_bindgen]
pub struct StreamHandle(Rc<RefCell<Option<InnerStream>>>);

#[wasm_bindgen]
impl StreamHandle {
    pub fn get_audio_track(&self) -> Result<Option<MediaStreamTrack>, JsValue> {
        match self.0.borrow().as_ref() {
            Some(inner) => Ok(inner.audio.as_ref().cloned()),
            None => Err(WasmErr::from_str("Detached state").into()),
        }
    }

    pub fn get_video_track(&self) -> Result<Option<MediaStreamTrack>, JsValue> {
        match self.0.borrow().as_ref() {
            Some(inner) => Ok(inner.video.as_ref().cloned()),
            None => Err(WasmErr::from_str("Detached state").into()),
        }
    }
}

impl TryFrom<MediaStream> for InnerStream {
    type Error = WasmErr;

    fn try_from(media_stream: MediaStream) -> Result<Self, Self::Error> {
        let mut stream = InnerStream {
            audio: None,
            video: None,
        };

        let audio_tracks = js_sys::try_iter(&media_stream.get_audio_tracks())?
            .ok_or_else(|| {
                WasmErr::from_str("MediaStream.get_audio_tracks() != Array")
            })?;

        for track in audio_tracks {
            let track = MediaStreamTrack::from(track?);

            if stream.audio.replace(track).is_some() {
                Err(WasmErr::from_str(
                    "Can handle only one audio track per stream",
                ))?;
            }
        }

        let video_tracks = js_sys::try_iter(&media_stream.get_video_tracks())?
            .ok_or_else(|| {
                WasmErr::from_str("MediaStream.get_video_tracks() != Array")
            })?;

        for track in video_tracks {
            let track = MediaStreamTrack::from(track?);

            if stream.video.replace(track).is_some() {
                Err(WasmErr::from_str(
                    "Can handle only one video track per stream",
                ))?;
            }
        }

        Ok(stream)
    }
}
