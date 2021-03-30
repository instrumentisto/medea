//! Wrapper around [`sys::MediaStreamTrack`] received from the remote.

use std::rc::Rc;

use futures::StreamExt;
use medea_client_api_proto::MediaSourceKind;
use medea_reactive::ObservableCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys as sys;

use crate::{
    media::MediaKind,
    utils::{Callback0, EventListener},
    JsMediaSourceKind,
};

/// Inner reference-counted data of [`Track`].
struct Inner {
    /// Underlying JS-side [`sys::MediaStreamTrack`].
    track: Rc<sys::MediaStreamTrack>,

    /// Listener for [ended][1] event;
    ///
    /// [1]: https://tinyurl.com/w3-streams#event-mediastreamtrack-ended
    on_ended: Option<EventListener<sys::MediaStreamTrack, sys::Event>>,

    /// Underlying [`sys::MediaStreamTrack`] kind.
    kind: MediaKind,

    /// Underlying [`sys::MediaStreamTrack`] source kind.
    media_source_kind: MediaSourceKind,

    /// Callback to be invoked when this [`Track`] is enabled.
    on_enabled: Callback0,

    /// Callback to be invoked when this [`Track`] is disabled.
    on_disabled: Callback0,

    /// Callback to be invoked when this [`Track`] is stopped.
    on_stopped: Rc<Callback0>,

    /// [`enabled`][1] property of [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
    enabled: ObservableCell<bool>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.on_ended.take();
    }
}

/// Wrapper around [MediaStreamTrack][1] received from the remote.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[wasm_bindgen(js_name = RemoteMediaTrack)]
#[derive(Clone)]
pub struct Track(Rc<Inner>);

impl Track {
    /// Creates a new [`Track`] spawning a listener for its [`enabled`][1]
    /// property changes.
    ///
    /// # Panics
    ///
    /// If provided [`sys::MediaStreamTrack`] kind is not `audio` or `video`.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    #[must_use]
    pub fn new<T>(track: T, media_source_kind: MediaSourceKind) -> Self
    where
        sys::MediaStreamTrack: From<T>,
    {
        let track = Rc::new(sys::MediaStreamTrack::from(track));
        let kind = match track.kind().as_ref() {
            "audio" => MediaKind::Audio,
            "video" => MediaKind::Video,
            _ => unreachable!(),
        };

        let on_stopped = Rc::new(Callback0::default());
        let on_ended = EventListener::new_once(Rc::clone(&track), "ended", {
            let on_stopped = Rc::clone(&on_stopped);
            let track = Rc::clone(&track);
            move |_| {
                if track.ready_state() == sys::MediaStreamTrackState::Live {
                    // Not supposed to ever happen, but call `on_stopped` just
                    // in case.
                    log::error!("Unexpected track stop: {}", track.id());
                    drop(on_stopped.call());
                }
            }
        })
        .unwrap();

        let track = Track(Rc::new(Inner {
            enabled: ObservableCell::new(track.enabled()),
            on_enabled: Callback0::default(),
            on_disabled: Callback0::default(),
            on_stopped,
            on_ended: Some(on_ended),
            media_source_kind,
            kind,
            track,
        }));

        let mut track_enabled_state_changes =
            track.enabled().subscribe().skip(1);
        spawn_local({
            let weak_inner = Rc::downgrade(&track.0);
            async move {
                while let Some(enabled) =
                    track_enabled_state_changes.next().await
                {
                    if let Some(track) = weak_inner.upgrade() {
                        if enabled {
                            track.on_enabled.call();
                        } else {
                            track.on_disabled.call();
                        }
                    } else {
                        break;
                    }
                }
            }
        });

        track
    }

    /// Indicates whether this [`Track`] is enabled.
    #[inline]
    #[must_use]
    pub fn enabled(&self) -> &ObservableCell<bool> {
        &self.0.enabled
    }

    /// Sets [`Track::enabled`] to the provided value.
    ///
    /// Updates [`enabled`][1] property in the underlying
    /// [`sys::MediaStreamTrack`].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.0.enabled.set(enabled);
        self.0.track.set_enabled(enabled);
    }

    /// Returns [`id`][1] of underlying [`sys::MediaStreamTrack`] of this
    /// [`Track`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    #[inline]
    #[must_use]
    pub fn id(&self) -> String {
        self.0.track.id()
    }

    /// Returns this [`Track`]'s kind (audio/video).
    #[inline]
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        self.0.kind
    }

    /// Returns this [`Track`]'s media source kind.
    #[inline]
    #[must_use]
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.0.media_source_kind
    }

    /// Stops track invoking `on_stopped` callback if track is in
    /// [`sys::MediaStreamTrackState::Live`] state.
    #[inline]
    pub fn stop(self) {
        if self.0.track.ready_state() == sys::MediaStreamTrackState::Live {
            self.0.track.stop();
            self.0.on_stopped.call();
        }
    }
}

#[wasm_bindgen(js_class = RemoteMediaTrack)]
impl Track {
    /// Returns the underlying [`sys::MediaStreamTrack`] of this [`Track`].
    #[must_use]
    pub fn get_track(&self) -> sys::MediaStreamTrack {
        Clone::clone(&self.0.track)
    }

    /// Indicate whether this [`Track`] is enabled.
    #[must_use]
    #[wasm_bindgen(js_name = enabled)]
    pub fn js_enabled(&self) -> bool {
        self.0.enabled.get()
    }

    /// Sets callback to invoke when this [`Track`] is enabled.
    pub fn on_enabled(&self, callback: js_sys::Function) {
        self.0.on_enabled.set_func(callback);
    }

    /// Sets callback to invoke when this [`Track`] is disabled.
    pub fn on_disabled(&self, callback: js_sys::Function) {
        self.0.on_disabled.set_func(callback);
    }

    /// Sets callback to invoke when this [`Track`] is stopped.
    pub fn on_stopped(&self, callback: js_sys::Function) {
        self.0.on_stopped.set_func(callback);
    }

    /// Returns [`MediaKind::Audio`] if this [`Track`] represents an audio
    /// track, or [`MediaKind::Video`] if it represents a video track.
    #[must_use]
    #[wasm_bindgen(js_name = kind)]
    pub fn js_kind(&self) -> MediaKind {
        self.kind()
    }

    /// Returns [`JsMediaSourceKind::Device`] if this [`Track`] is sourced from
    /// some device (webcam/microphone), or [`JsMediaSourceKind::Display`] if
    /// it's captured via [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    #[must_use]
    #[wasm_bindgen(js_name = media_source_kind)]
    pub fn js_media_source_kind(&self) -> JsMediaSourceKind {
        self.0.media_source_kind.into()
    }
}
