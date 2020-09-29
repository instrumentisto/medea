//! [MediaStreamTrack][1] related objects.
//!
//! [1]: https://developer.mozilla.org/en-US/docs/Web/API/MediaStreamTrack

use std::rc::{Rc, Weak};

use derive_more::Display;
use futures::StreamExt;
use medea_reactive::ObservableCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::MediaStreamTrack as SysMediaStreamTrack;

use crate::utils::Callback0;

/// [MediaStreamTrack.kind][1] representation.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-kind
#[derive(Clone, Copy, Debug, Eq, PartialEq, Display)]
pub enum TrackKind {
    /// Audio track.
    #[display(fmt = "audio")]
    Audio,

    /// Video track.
    #[display(fmt = "video")]
    Video,
}

/// Wrapper around [`SysMediaStreamTrack`] to track when it's enabled or
/// disabled.
struct InnerMediaStreamTrack {
    /// Underlying JS-side [`SysMediaStreamTrack`].
    track: SysMediaStreamTrack,

    /// Flag which indicates that this [`MediaStreamTrack`] was received from
    /// `getDisplayMedia`.
    is_display: bool,

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
/// Track will be automatically stopped when there are no strong references
/// left.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[wasm_bindgen(js_name = MediaTrack)]
#[derive(Clone)]
pub struct MediaStreamTrack(Rc<InnerMediaStreamTrack>);

impl MediaStreamTrack {
    /// Creates new [`MediaStreamTrack`], spawns listener for
    /// [`InnerMediaStreamTrack::enabled`] state changes.
    pub fn new<T>(track: T, is_display: bool) -> Self
    where
        SysMediaStreamTrack: From<T>,
    {
        let track = SysMediaStreamTrack::from(track);
        let track = MediaStreamTrack(Rc::new(InnerMediaStreamTrack {
            enabled: ObservableCell::new(track.enabled()),
            on_enabled: Callback0::default(),
            on_disabled: Callback0::default(),
            is_display,
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

    /// Returns `true` if this [`MediaStreamTrack`] is enabled.
    #[inline]
    pub fn enabled(&self) -> &ObservableCell<bool> {
        &self.0.enabled
    }

    /// Sets [`MediaStreamTrack::enabled`] to the provided value.
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
    pub fn kind(&self) -> TrackKind {
        match self.0.track.kind().as_ref() {
            "audio" => TrackKind::Audio,
            "video" => TrackKind::Video,
            _ => unreachable!(),
        }
    }

    /// Creates weak reference to underlying [MediaStreamTrack][2].
    ///
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn downgrade(&self) -> WeakMediaStreamTrack {
        WeakMediaStreamTrack(Rc::downgrade(&self.0))
    }
}

#[wasm_bindgen(js_class = MediaTrack)]
impl MediaStreamTrack {
    /// Returns underlying [`SysMediaStreamTrack`] from this
    /// [`MediaStreamTrack`].
    pub fn get_track(&self) -> SysMediaStreamTrack {
        Clone::clone(&self.0.track)
    }

    /// Returns is this [`MediaStreamTrack`] enabled.
    #[wasm_bindgen(js_name = enabled)]
    pub fn js_enabled(&self) -> bool {
        self.0.enabled.get()
    }

    /// Sets callback, which will be invoked when this [`MediaStreamTrack`] will
    /// be enabled.
    pub fn on_enabled(&self, callback: js_sys::Function) {
        self.0.on_enabled.set_func(callback);
    }

    /// Sets callback, which will be invoked when this [`MediaStreamTrack`] will
    /// be enabled.
    pub fn on_disabled(&self, callback: js_sys::Function) {
        self.0.on_disabled.set_func(callback);
    }

    /// Returns [`TrackKind`] of this [`MediaStreamTrack`] converted to
    /// [`String`].
    #[wasm_bindgen(js_name = kind)]
    pub fn js_kind(&self) -> String {
        self.kind().to_string()
    }

    /// Returns `true` if this [`MediaStreamTrack`] was received from
    /// `getDisplayMedia`.
    pub fn is_display(&self) -> bool {
        self.0.is_display
    }
}

impl AsRef<SysMediaStreamTrack> for MediaStreamTrack {
    #[inline]
    fn as_ref(&self) -> &SysMediaStreamTrack {
        &self.0.track
    }
}

impl Drop for MediaStreamTrack {
    #[inline]
    fn drop(&mut self) {
        // Last strong ref being dropped, so stop underlying MediaTrack
        if Rc::strong_count(&self.0) == 1 {
            self.0.track.stop();
        }
    }
}

/// Weak reference to [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
pub struct WeakMediaStreamTrack(Weak<InnerMediaStreamTrack>);

impl WeakMediaStreamTrack {
    /// Tries to upgrade this weak reference to a strong one.
    #[inline]
    pub fn upgrade(&self) -> Option<MediaStreamTrack> {
        self.0.upgrade().map(MediaStreamTrack)
    }

    /// Checks whether this weak reference can be upgraded to a strong one.
    #[inline]
    pub fn can_be_upgraded(&self) -> bool {
        self.0.strong_count() > 0
    }
}
