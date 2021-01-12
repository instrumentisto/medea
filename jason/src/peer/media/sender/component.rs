//! Implementation of [`Component`] for `MediaTrack` with a `Send` direction.

use std::{cell::Cell, rc::Rc};

use medea_client_api_proto::{
    MediaSourceKind, MediaType, MemberId, TrackId, TrackPatchEvent,
};
use medea_macro::watchers;
use medea_reactive::{AllProcessed, Guarded, ProgressableCell};

use crate::{
    media::LocalTracksConstraints,
    peer::{media::Result, MediaConnectionsError},
    utils::component,
    MediaKind,
};

use super::Sender;

/// Component responsible for the [`Sender`] enabling/disabling and
/// muting/unmuting.
pub type Component = component::Component<State, Sender>;

/// State of the [`Component`].
#[derive(Debug)]
pub struct State {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    receivers: Vec<MemberId>,
    enabled_individual: ProgressableCell<bool>,
    enabled_general: ProgressableCell<bool>,
    muted: ProgressableCell<bool>,
    need_local_stream_update: Cell<bool>,
}

impl State {
    /// Creates new [`State`] with a provided data.
    /// # Errors
    ///
    /// Returns [`MediaConnectionsError::CannotDisableRequiredSender`] if this
    /// [`Sender`] can't be disabled.
    pub fn new(
        id: TrackId,
        mid: Option<String>,
        media_type: MediaType,
        receivers: Vec<MemberId>,
        send_constraints: &LocalTracksConstraints,
    ) -> Result<Self> {
        let required = media_type.required();
        let enabled = send_constraints.enabled(&media_type);
        let muted = send_constraints.muted(&media_type);
        if (muted || !enabled) && required {
            return Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
            ));
        }

        Ok(Self {
            id,
            mid,
            media_type,
            receivers,
            enabled_general: ProgressableCell::new(enabled),
            enabled_individual: ProgressableCell::new(enabled),
            muted: ProgressableCell::new(muted),
            need_local_stream_update: Cell::new(false),
        })
    }

    /// Returns [`TrackId`] of this [`State`].
    #[inline]
    pub fn id(&self) -> TrackId {
        self.id
    }

    /// Returns current `mid` of this [`State`].
    #[inline]
    pub fn mid(&self) -> &Option<String> {
        &self.mid
    }

    /// Returns current [`MediaType`] of this [`State`].
    #[inline]
    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    /// Returns current [`MemberId`]s of the `Member`s to which this
    /// [`State`] should send media data.
    #[inline]
    pub fn receivers(&self) -> &Vec<MemberId> {
        &self.receivers
    }

    /// Returns current individual media exchange state of this [`State`].
    #[inline]
    pub fn is_enabled_individual(&self) -> bool {
        self.enabled_individual.get()
    }

    /// Returns current general media exchange state of this [`State`].
    #[inline]
    pub fn is_enabled_general(&self) -> bool {
        self.enabled_general.get()
    }

    /// Returns current mute state of this [`State`].
    #[inline]
    pub fn is_muted(&self) -> bool {
        self.muted.get()
    }

    /// Updates this [`State`] with a provided [`TrackPatchEvent`].
    pub fn update(&self, track_patch: &TrackPatchEvent) {
        if track_patch.id != self.id {
            return;
        }
        if let Some(enabled_general) = track_patch.enabled_general {
            self.enabled_general.set(enabled_general);
        }
        if let Some(enabled_individual) = track_patch.enabled_individual {
            self.enabled_individual.set(enabled_individual);
        }
        if let Some(muted) = track_patch.muted {
            self.muted.set(muted);
        }
    }

    /// Returns [`Future`] which will be resolved when [`State`] update
    /// will be applied on [`Sender`].
    ///
    /// [`Future`]: std::future::Future
    pub fn when_updated(&self) -> AllProcessed<'static, ()> {
        medea_reactive::when_all_processed(vec![
            self.enabled_general.when_all_processed().into(),
            self.enabled_individual.when_all_processed().into(),
            self.muted.when_all_processed().into(),
        ])
    }

    /// Returns `true` if local `MediaStream` update needed for this
    /// [`State`].
    #[inline]
    pub fn is_local_stream_update_needed(&self) -> bool {
        self.need_local_stream_update.get()
    }

    /// Sets [`State::need_local_stream_update`] to `false`.
    #[inline]
    pub fn local_stream_updated(&self) {
        self.need_local_stream_update.set(false);
    }

    /// Returns [`MediaKind`] of this [`State`].
    #[inline]
    pub fn media_kind(&self) -> MediaKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaKind::Audio,
            MediaType::Video(_) => MediaKind::Video,
        }
    }

    /// Returns [`MediaSourceKind`] of this [`State`].
    #[inline]
    pub fn media_source(&self) -> MediaSourceKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaSourceKind::Device,
            MediaType::Video(video) => video.source_kind,
        }
    }
}

#[watchers]
impl Component {
    /// Watcher for the [`State::enabled_individual`] update.
    ///
    /// Calls [`Sender::set_enabled_individual`] with a new value.
    ///
    /// If new value is `true` then sets
    /// [`State::need_local_stream_update`] flag to `true`, otherwise
    /// calls [`Sender::remove_track`].
    #[watch(self.enabled_individual.subscribe())]
    #[inline]
    async fn enabled_individual_watcher(
        sender: Rc<Sender>,
        state: Rc<State>,
        enabled_individual: Guarded<bool>,
    ) -> Result<()> {
        sender.set_enabled_individual(*enabled_individual);
        if *enabled_individual {
            state.need_local_stream_update.set(true);
        } else {
            sender.remove_track().await;
        }

        Ok(())
    }

    /// Watcher for the [`State::enabled_general`] update.
    ///
    /// Calls [`Sender::set_enabled_general_state`] with a new value.
    #[watch(self.enabled_general.subscribe())]
    #[inline]
    async fn enabled_general_watcher(
        sender: Rc<Sender>,
        _: Rc<State>,
        enabled_general: Guarded<bool>,
    ) -> Result<()> {
        sender.set_enabled_general(*enabled_general);

        Ok(())
    }

    /// Watcher for the [`State::muted`] update.
    ///
    /// Calls [`Sender::set_muted`] with a new value.
    #[watch(self.muted.subscribe())]
    #[inline]
    async fn muted_watcher(
        sender: Rc<Sender>,
        _: Rc<State>,
        muted: Guarded<bool>,
    ) -> Result<()> {
        sender.set_muted(*muted);

        Ok(())
    }
}
