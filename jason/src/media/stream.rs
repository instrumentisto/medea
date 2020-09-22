//! [MediaStream][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastream

use std::rc::{Rc, Weak};

use derive_more::{AsRef, Display};
use medea_reactive::ObservableCell;
use wasm_bindgen::prelude::*;
use web_sys::{
    MediaStream as SysMediaStream, MediaStreamTrack as SysMediaStreamTrack,
};

use crate::{
    utils::{Callback0, HandlerDetachedError},
    MediaStreamSettings,
};
use futures::StreamExt;
use wasm_bindgen_futures::spawn_local;

/// Representation of [MediaStream][1] object. Contains strong references to
/// [`MediaStreamTrack`].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
#[wasm_bindgen(js_name = LocalMediaStream)]
#[derive(AsRef, Clone)]
pub struct MediaStream {
    #[as_ref]
    stream: SysMediaStream,
    constraints: MediaStreamSettings,
    tracks: Vec<MediaStreamTrack>,
}

impl MediaStream {
    /// Creates new [`MediaStream`] from provided tracks and stream settings.
    pub fn new(
        tracks: Vec<MediaStreamTrack>,
        constraints: MediaStreamSettings,
    ) -> Self {
        let stream = SysMediaStream::new().unwrap();
        tracks
            .iter()
            .for_each(|track| stream.add_track(track.as_ref()));

        MediaStream {
            stream,
            constraints,
            tracks,
        }
    }

    /// Consumes `self` returning all underlying [`MediaStreamTrack`]s.
    pub fn into_tracks(self) -> Vec<MediaStreamTrack> {
        self.tracks
    }
}

#[wasm_bindgen(js_class = LocalMediaStream)]
impl MediaStream {
    /// Returns underlying [MediaStream][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    pub fn get_media_stream(&self) -> SysMediaStream {
        Clone::clone(&self.stream)
    }

    /// Drops all audio tracks contained in ths stream.
    pub fn free_audio(&mut self) {
        self.tracks.retain(|track| match track.kind() {
            TrackKind::Audio => false,
            TrackKind::Video => true,
        });
    }

    /// Drops all video tracks contained in ths stream.
    pub fn free_video(&mut self) {
        self.tracks.retain(|track| match track.kind() {
            TrackKind::Audio => true,
            TrackKind::Video => false,
        });
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

/// Wrapper around [`SysMediaStreamTrack`] to track when it's enabled or
/// disabled.
struct InnerMediaStreamTrack {
    /// Underlying JS-side [`SysMediaStreamTrack`].
    track: SysMediaStreamTrack,

    on_enabled: Callback0,
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
#[derive(Clone)]
pub struct MediaStreamTrack(Rc<InnerMediaStreamTrack>);

impl MediaStreamTrack {
    pub fn new<T>(track: T) -> Self
    where
        SysMediaStreamTrack: From<T>,
    {
        let track = SysMediaStreamTrack::from(track);
        let track = MediaStreamTrack(Rc::new(InnerMediaStreamTrack {
            enabled: ObservableCell::new(track.enabled()),
            on_enabled: Callback0::default(),
            on_disabled: Callback0::default(),
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

    pub fn new_handle(self) -> MediaStreamTrackHandle {
        MediaStreamTrackHandle(self)
    }
}

#[wasm_bindgen]
pub struct MediaStreamTrackHandle(MediaStreamTrack);

#[wasm_bindgen]
impl MediaStreamTrackHandle {
    pub fn get_track(&self) -> SysMediaStreamTrack {
        Clone::clone(&self.0.0.track)
    }

    pub fn on_enabled(&self, callback: js_sys::Function) {
        self.0.0.on_enabled.set_func(callback);
    }

    pub fn on_disabled(&self, callback: js_sys::Function) {
        self.0.0.on_disabled.set_func(callback);
    }

    pub fn kind(&self) -> String {
        self.0.kind().to_string()
    }
}

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

impl AsRef<SysMediaStreamTrack> for MediaStreamTrack {
    #[inline]
    fn as_ref(&self) -> &SysMediaStreamTrack {
        &self.0.track
    }
}

impl MediaStreamTrack {
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

impl Drop for MediaStreamTrack {
    #[inline]
    fn drop(&mut self) {
        // Last strong ref being dropped, so stop underlying MediaTrack
        log::debug!(
            "Trying to drop. Count of refs: {}",
            Rc::strong_count(&self.0)
        );
        if Rc::strong_count(&self.0) == 1 {
            log::debug!("Dropped MediaStreamTrack");
            self.0.track.stop();
        }
    }
}
