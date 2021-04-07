//! Wrapper around a received remote [`platform::MediaStreamTrack`].

use std::rc::Rc;

use futures::StreamExt;
use medea_client_api_proto as proto;
use medea_reactive::ObservableCell;

use crate::{
    media::{MediaKind, MediaSourceKind},
    platform,
};

/// Inner reference-counted data of a [`Track`].
struct Inner {
    /// Underlying platform-specific [`platform::MediaStreamTrack`].
    track: platform::MediaStreamTrack,

    /// Listener for an [ended][1] event.
    ///
    /// [1]: https://tinyurl.com/w3-streams#event-mediastreamtrack-ended
    on_ended: Option<EventListener<platform::MediaStreamTrack, sys::Event>>,

    /// Underlying [`platform::MediaStreamTrack`] source kind.
    media_source_kind: proto::MediaSourceKind,

    /// Callback invoked when this [`Track`] is enabled.
    on_enabled: platform::Callback<()>,

    /// Callback invoked when this [`Track`] is disabled.
    on_disabled: platform::Callback<()>,

    /// Callback to be invoked when this [`Track`] is stopped.
    on_stopped: Rc<platform::Callback<()>>,

    /// [`enabled`][1] property of this [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
    enabled: ObservableCell<bool>,
}

impl Drop for Inner {
    #[inline]
    fn drop(&mut self) {
        self.on_ended.take();
    }
}

/// Wrapper around a received remote [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[derive(Clone)]
pub struct Track(Rc<Inner>);

impl Track {
    /// Creates a new [`Track`] spawning a listener for its [`enabled`][1]
    /// property changes.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    #[must_use]
    pub fn new<T>(track: T, media_source_kind: proto::MediaSourceKind) -> Self
    where
        platform::MediaStreamTrack: From<T>,
    {
        let track = platform::MediaStreamTrack::from(track);

        let on_stopped = Rc::new(platform::Callback::default());
        let on_ended = EventListener::new_once(Rc::clone(&track), "ended", {
            let on_stopped = Rc::clone(&on_stopped);
            let track = Rc::clone(&track);
            move |_| {
                // Not supposed to ever happen, but call `on_stopped` just
                // in case.
                log::error!("Unexpected track stop: {}", track.id());
                drop(on_stopped.call());
            }
        })
        .unwrap();

        let track = Track(Rc::new(Inner {
            enabled: ObservableCell::new(track.enabled()),
            on_enabled: platform::Callback::default(),
            on_disabled: platform::Callback::default(),
            on_stopped,
            on_ended: Some(on_ended),
            media_source_kind,
            track,
        }));

        let mut track_enabled_state_changes =
            track.0.enabled.subscribe().skip(1);
        platform::spawn({
            let weak_inner = Rc::downgrade(&track.0);
            async move {
                while let Some(enabled) =
                    track_enabled_state_changes.next().await
                {
                    if let Some(track) = weak_inner.upgrade() {
                        if enabled {
                            track.on_enabled.call0();
                        } else {
                            track.on_disabled.call0();
                        }
                    } else {
                        break;
                    }
                }
            }
        });

        track
    }

    /// Sets [`Track::enabled`] to the provided value.
    ///
    /// Updates [`enabled`][1] property in the underlying
    /// [`platform::MediaStreamTrack`].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.0.enabled.set(enabled);
        self.0.track.set_enabled(enabled);
    }

    /// Returns [`id`][1] of the underlying [`platform::MediaStreamTrack`] of
    /// this [`Track`].
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
        self.0.track.kind()
    }

    /// Returns this [`Track`]'s media source kind.
    #[inline]
    #[must_use]
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.0.media_source_kind.into()
    }

    /// Stops this [`Track`] invoking an `on_stopped` callback if it's in a
    /// [`sys::MediaStreamTrackState::Live`] state.
    #[inline]
    pub fn stop(self) {
        if self.0.track.ready_state() == sys::MediaStreamTrackState::Live {
            self.0.track.stop();
            self.0.on_stopped.call();
        }
    }

    /// Returns the underlying [`platform::MediaStreamTrack`] of this [`Track`].
    #[inline]
    #[must_use]
    pub fn get_track(&self) -> &platform::MediaStreamTrack {
        &self.0.track
    }

    /// Indicates whether this [`Track`] is enabled.
    #[inline]
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.0.enabled.get()
    }

    /// Sets callback, invoked when this [`Track`] is enabled.
    #[inline]
    pub fn on_enabled(&self, callback: platform::Function<()>) {
        self.0.on_enabled.set_func(callback);
    }

    /// Sets callback, invoked when this [`Track`] is disabled.
    #[inline]
    pub fn on_disabled(&self, callback: platform::Function<()>) {
        self.0.on_disabled.set_func(callback);
    }

    /// Sets callback to invoke when this [`Track`] is stopped.
    #[inline]
    pub fn on_stopped(&self, callback: platform::Function<()>) {
        self.0.on_stopped.set_func(callback);
    }
}
