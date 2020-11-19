//! Implementation of the wrapper around [`SysMediaStreamTrack`] received from
//! the remote side.

use std::rc::Rc;

use futures::StreamExt;
use medea_client_api_proto::MediaSourceKind;
use medea_reactive::ObservableCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::MediaStreamTrack as SysMediaStreamTrack;

use crate::{media::MediaKind, utils::Callback0, JsMediaSourceKind};

/// Wrapper around [`SysMediaStreamTrack`] received from the remote side to
/// track when it's enabled or disabled.
struct Inner {
    /// Underlying JS-side [`SysMediaStreamTrack`].
    track: SysMediaStreamTrack,

    /// Underlying [`SysMediaStreamTrack`] kind.
    kind: MediaKind,

    /// Underlying [`SysMediaStreamTrack`] source kind.
    media_source_kind: MediaSourceKind,

    /// Callback to be invoked when this [`MediaStreamTrack`] is enabled.
    on_enabled: Callback0,

    /// Callback to be invoked when this [`MediaStreamTrack`] is disabled.
    on_disabled: Callback0,

    /// [enabled] property of [MediaStreamTrack][1].
    ///
    /// [enabled]: https://tinyurl.com/y5byqdea
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
    enabled: ObservableCell<bool>,
}

/// Strong reference to [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[wasm_bindgen(js_name = RemoteMediaTrack)]
#[derive(Clone)]
pub struct Track(Rc<Inner>);

impl Track {
    /// Creates new [`Track`], spawns listener for
    /// [`Inner::enabled`] state changes.
    pub fn new<T>(track: T, media_source_kind: MediaSourceKind) -> Self
    where
        SysMediaStreamTrack: From<T>,
    {
        let track = SysMediaStreamTrack::from(track);
        let kind = match track.kind().as_ref() {
            "audio" => MediaKind::Audio,
            "video" => MediaKind::Video,
            _ => unreachable!(),
        };

        let track = Track(Rc::new(Inner {
            enabled: ObservableCell::new(track.enabled()),
            on_enabled: Callback0::default(),
            on_disabled: Callback0::default(),
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

    /// Returns `true` if this [`Track`] is enabled.
    #[inline]
    pub fn enabled(&self) -> &ObservableCell<bool> {
        &self.0.enabled
    }

    /// Sets [`Track::enabled`] to the provided value.
    ///
    /// Updates `enabled` in the underlying [`SysMediaStreamTrack`].
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.0.enabled.set(enabled);
        self.0.track.set_enabled(enabled);
    }

    /// Returns [`id`][1] of underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn id(&self) -> String {
        self.0.track.id()
    }

    /// Returns track kind (audio/video).
    #[inline]
    pub fn kind(&self) -> MediaKind {
        self.0.kind
    }

    /// Returns this [`Track`] media source kind.
    #[inline]
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.0.media_source_kind
    }
}

#[wasm_bindgen(js_class = RemoteMediaTrack)]
impl Track {
    /// Returns underlying [`SysMediaStreamTrack`] from this
    /// [`Track`].
    pub fn get_track(&self) -> SysMediaStreamTrack {
        Clone::clone(&self.0.track)
    }

    /// Returns is this [`Track`] enabled.
    #[wasm_bindgen(js_name = enabled)]
    pub fn js_enabled(&self) -> bool {
        self.0.enabled.get()
    }

    /// Sets callback, which will be invoked when this [`Track`]
    /// is enabled.
    pub fn on_enabled(&self, callback: js_sys::Function) {
        self.0.on_enabled.set_func(callback);
    }

    /// Sets callback, which will be invoked when this [`Track`]
    /// is enabled.
    pub fn on_disabled(&self, callback: js_sys::Function) {
        self.0.on_disabled.set_func(callback);
    }

    /// Returns a [`String`] set to `audio` if the track is an audio track
    /// and to `video`, if it is a video track.
    #[wasm_bindgen(js_name = kind)]
    pub fn js_kind(&self) -> MediaKind {
        self.kind()
    }

    /// Returns a [`String`] set to `device` if track is sourced from some
    /// device (webcam/microphone) and to `display`, if track is captured
    /// via [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://tinyurl.com/y2anfntz
    #[wasm_bindgen(js_name = media_source_kind)]
    pub fn js_media_source_kind(&self) -> JsMediaSourceKind {
        self.0.media_source_kind.into()
    }
}
