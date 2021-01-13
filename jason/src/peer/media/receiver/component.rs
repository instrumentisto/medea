//! [`Component`] for `MediaTrack` with a `Recv` direction.

use std::rc::Rc;

use medea_client_api_proto::{MediaType, MemberId, TrackId, TrackPatchEvent};
use medea_macro::watchers;
use medea_reactive::{AllProcessed, Guarded, ProgressableCell};

use crate::{media::RecvConstraints, peer::media::Result, utils::component};

use super::Receiver;

/// Component responsible for the [`Receiver`] enabling/disabling and
/// muting/unmuting.
pub type Component = component::Component<State, Receiver>;

/// State of the [`Component`].
#[derive(Debug)]
pub struct State {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    sender_id: MemberId,
    enabled_individual: ProgressableCell<bool>,
    enabled_general: ProgressableCell<bool>,
    muted: ProgressableCell<bool>,
}

impl State {
    /// Returns [`State`] with the provided data.
    #[must_use]
    pub fn new(
        id: TrackId,
        mid: Option<String>,
        media_type: MediaType,
        sender: MemberId,
        recv_constraints: &RecvConstraints,
    ) -> Self {
        let enabled = match &media_type {
            MediaType::Audio(_) => recv_constraints.is_audio_enabled(),
            MediaType::Video(_) => recv_constraints.is_video_enabled(),
        };
        Self {
            id,
            mid,
            media_type,
            sender_id: sender,
            enabled_general: ProgressableCell::new(enabled),
            enabled_individual: ProgressableCell::new(enabled),
            muted: ProgressableCell::new(false),
        }
    }

    /// Returns [`TrackId`] of this [`State`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> TrackId {
        self.id
    }

    /// Returns current `mid` of this [`State`].
    #[inline]
    #[must_use]
    pub fn mid(&self) -> Option<&str> {
        self.mid.as_deref()
    }

    /// Returns current [`MediaType`] of this [`State`].
    #[inline]
    #[must_use]
    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    /// Returns current [`MemberId`] of the `Member` from which this
    /// [`State`] should receive media data.
    #[inline]
    #[must_use]
    pub fn sender_id(&self) -> &MemberId {
        &self.sender_id
    }

    /// Returns current individual media exchange state of this [`State`].
    #[inline]
    #[must_use]
    pub fn enabled_individual(&self) -> bool {
        self.enabled_individual.get()
    }

    /// Returns current general media exchange state of this [`State`].
    #[inline]
    #[must_use]
    pub fn enabled_general(&self) -> bool {
        self.enabled_general.get()
    }

    /// Updates this [`State`] with the provided [`TrackPatchEvent`].
    pub fn update(&self, track_patch: &TrackPatchEvent) {
        if self.id != track_patch.id {
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

    /// Returns [`Future`] resolving when [`State`] update will be applied onto
    /// [`Receiver`].
    ///
    /// [`Future`]: std::future::Future
    pub fn when_updated(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![
            self.enabled_general.when_all_processed().into(),
            self.enabled_individual.when_all_processed().into(),
            self.muted.when_all_processed().into(),
        ])
    }
}

#[watchers]
impl Component {
    /// Watcher for the [`State::muted`] update.
    ///
    /// Calls [`Receiver::set_muted()`] with a new value.
    #[inline]
    #[watch(self.muted.subscribe())]
    async fn muted_state_changed(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        muted: Guarded<bool>,
    ) -> Result<()> {
        receiver.set_muted(*muted);
        Ok(())
    }

    /// Watcher for the [`State::enabled_individual`] update.
    ///
    /// Calls [`Receiver::set_enabled_individual_state()`] with a new value.
    #[inline]
    #[watch(self.enabled_individual.subscribe())]
    async fn enabled_individual_changed(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        enabled_individual: Guarded<bool>,
    ) -> Result<()> {
        receiver.set_enabled_individual_state(*enabled_individual);
        Ok(())
    }

    /// Watcher for the [`State::enabled_general`] update.
    ///
    /// Calls [`Receiver::set_enabled_general_state()`] with a new value.
    #[inline]
    #[watch(self.enabled_general.subscribe())]
    async fn enabled_general_changed(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        enabled_general: Guarded<bool>,
    ) -> Result<()> {
        receiver.set_enabled_general_state(*enabled_general);
        Ok(())
    }
}
