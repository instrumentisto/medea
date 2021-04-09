//! Wrapper around a received remote [`platform::MediaStreamTrack`].

use std::rc::Rc;

use futures::StreamExt as _;
use medea_client_api_proto as proto;
use medea_reactive::ObservableCell;

use crate::{
    media::{track::MediaStreamTrackState, MediaKind, MediaSourceKind},
    platform,
};

/// Inner reference-counted data of a [`Track`].
struct Inner {
    /// Underlying platform-specific [`platform::MediaStreamTrack`].
    track: platform::MediaStreamTrack,

    /// Underlying [`platform::MediaStreamTrack`] source kind.
    media_source_kind: proto::MediaSourceKind,

    /// Callback invoked when this [`Track`] is enabled.
    on_enabled: platform::Callback<()>,

    /// Callback invoked when this [`Track`] is disabled.
    on_disabled: platform::Callback<()>,

    /// Callback to be invoked when this [`Track`] is muted.
    on_muted: platform::Callback<()>,

    /// Callback to be invoked when this [`Track`] is unmuted.
    on_unmuted: platform::Callback<()>,

    /// Callback to be invoked when this [`Track`] is stopped.
    on_stopped: platform::Callback<()>,

    /// Indicates whether this track is enabled, meaning that
    /// [RTCRtpTransceiver] that created this track has its direction set to
    /// [`sendrecv`][1] or [`recvonly`][2].
    ///
    /// Updating this value fires `on_enabled` or `on_disabled` callback and
    /// changes [`enabled`][3] property of the underlying
    /// [MediaStreamTrack][4].
    ///
    /// [RTCRtpTransceiver]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection-sendrecv
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection-revonly
    /// [3]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    /// [4]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
    enabled: ObservableCell<bool>,

    /// Indicates whether this track is muted.
    ///
    /// Updating this value fires `on_muted` or `on_unmuted` callback and
    /// changes [`enabled`][1] property of the underlying
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
    muted: ObservableCell<bool>,
}

/// Wrapper around a received remote [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[derive(Clone)]
pub struct Track(Rc<Inner>);

impl Track {
    /// Creates a new [`Track`].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    #[allow(clippy::mut_mut)]
    #[must_use]
    pub fn new<T>(
        track: T,
        media_source_kind: proto::MediaSourceKind,
        enabled: bool,
        muted: bool,
    ) -> Self
    where
        platform::MediaStreamTrack: From<T>,
    {
        let track = platform::MediaStreamTrack::from(track);
        let track = Track(Rc::new(Inner {
            track,
            media_source_kind,
            enabled: ObservableCell::new(enabled),
            muted: ObservableCell::new(muted),
            on_enabled: platform::Callback::default(),
            on_disabled: platform::Callback::default(),
            on_stopped: platform::Callback::default(),
            on_muted: platform::Callback::default(),
            on_unmuted: platform::Callback::default(),
        }));

        track.0.track.on_ended({
            let weak_inner = Rc::downgrade(&track.0);
            Some(move || {
                // Not supposed to ever happen, but call `on_stopped` just
                // in case.
                if let Some(inner) = weak_inner.upgrade() {
                    log::error!("Unexpected track stop: {}", inner.track.id());
                    inner.on_stopped.call0();
                }
            })
        });

        let mut enabled_changes = track.0.enabled.subscribe().skip(1).fuse();
        let mut muted_changes = track.0.muted.subscribe().skip(1).fuse();
        platform::spawn({
            enum TrackChange {
                Enabled(bool),
                Muted(bool),
            }

            let weak_inner = Rc::downgrade(&track.0);
            async move {
                loop {
                    let event = futures::select! {
                        enabled = enabled_changes.select_next_some() => {
                            TrackChange::Enabled(enabled)
                        },
                        muted = muted_changes.select_next_some() => {
                            TrackChange::Muted(muted)
                        },
                        complete => break,
                    };
                    if let Some(track) = weak_inner.upgrade() {
                        track.track.set_enabled(
                            track.enabled.get() && !track.muted.get(),
                        );
                        match event {
                            TrackChange::Enabled(enabled) => {
                                if enabled {
                                    track.on_enabled.call0();
                                } else {
                                    track.on_disabled.call0();
                                }
                            }
                            TrackChange::Muted(muted) => {
                                if muted {
                                    track.on_muted.call0();
                                } else {
                                    track.on_unmuted.call0();
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        });

        track
    }

    /// Sets `enabled` property on this [`Track`].
    ///
    /// Calls `on_enabled` or `or_disabled` callback respectively.
    ///
    /// Updates [`enabled`][1] property in the underlying
    /// [`platform::MediaStreamTrack`].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.0.enabled.set(enabled);
    }

    /// Sets `muted` property on this [`Track`].
    ///
    /// Calls `on_muted` or `or_unmuted` callback respectively.
    ///
    /// Updates [`enabled`][1] property in the underlying
    /// [`platform::MediaStreamTrack`].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    #[inline]
    pub fn set_muted(&self, muted: bool) {
        self.0.muted.set(muted);
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
    /// [`MediaStreamTrackState::Live`] state.
    #[inline]
    pub fn stop(self) {
        if self.0.track.ready_state() == MediaStreamTrackState::Live {
            self.0.on_stopped.call0();
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

    /// Indicate whether this [`Track`] is muted.
    #[inline]
    #[must_use]
    pub fn muted(&self) -> bool {
        self.0.muted.get()
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

    /// Sets callback to invoke when this [`Track`] is muted.
    #[inline]
    pub fn on_muted(&self, callback: platform::Function<()>) {
        self.0.on_muted.set_func(callback);
    }

    /// Sets callback to invoke when this [`Track`] is unmuted.
    #[inline]
    pub fn on_unmuted(&self, callback: platform::Function<()>) {
        self.0.on_unmuted.set_func(callback);
    }

    /// Sets callback to invoke when this [`Track`] is stopped.
    #[inline]
    pub fn on_stopped(&self, callback: platform::Function<()>) {
        self.0.on_stopped.set_func(callback);
    }
}
