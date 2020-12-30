//! Implementation of the [`ReceiverComponent`].

use std::rc::Rc;

use medea_client_api_proto::{MediaType, MemberId, TrackId, TrackPatchEvent};
use medea_macro::{watch, watchers};
use medea_reactive::{Guarded, ProgressableCell, RecheckableFutureExt};
use tracerr::Traced;

use crate::{
    media::RecvConstraints,
    peer::{MediaConnectionsError, Receiver},
    utils::component,
};

/// Component responsible for the [`Receiver`] enabling/disabling and
/// muting/unmuting.
pub type ReceiverComponent = component::Component<ReceiverState, Receiver>;

/// State of the [`ReceiverComponent`].
pub struct ReceiverState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    sender_id: MemberId,
    enabled_individual: ProgressableCell<bool>,
    enabled_general: ProgressableCell<bool>,
    muted: ProgressableCell<bool>,
}

impl ReceiverState {
    /// Returns [`ReceiverState`] with a provided data.
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

    /// Returns [`TrackId`] of this [`ReceiverState`].
    #[inline]
    pub fn id(&self) -> TrackId {
        self.id
    }

    /// Returns current `mid` of this [`ReceiverState`].
    #[inline]
    pub fn mid(&self) -> &Option<String> {
        &self.mid
    }

    /// Returns current [`MediaType`] of this [`ReceiverState`].
    #[inline]
    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    /// Returns current [`MemberId`] of the `Member` from which this
    /// [`ReceiverState`] should receive media data.
    #[inline]
    pub fn sender_id(&self) -> &MemberId {
        &self.sender_id
    }

    /// Returns current individual media exchange state of this
    /// [`ReceiverState`].
    #[inline]
    pub fn enabled_individual(&self) -> bool {
        self.enabled_individual.get()
    }

    /// Returns current general media exchange state of this [`ReceiverState`].
    #[inline]
    pub fn enabled_general(&self) -> bool {
        self.enabled_general.get()
    }

    /// Updates this [`ReceiverState`] with a provided [`TrackPatchEvent`].
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

    /// Returns [`Future`] which will be resolved when [`ReceiverState`] update
    /// will be applied on [`Receiver`].
    ///
    /// [`Future`]: std::future::Future
    pub fn when_updated(&self) -> impl RecheckableFutureExt<Output = ()> {
        medea_reactive::join_all(vec![
            self.enabled_general.when_all_processed(),
            self.enabled_individual.when_all_processed(),
            self.muted.when_all_processed(),
        ])
    }
}

#[watchers]
impl ReceiverComponent {
    /// Watcher for the [`ReceiverState::muted`] update.
    ///
    /// Calls [`Receiver::set_muted`] with a new value.
    #[watch(self.state().muted.subscribe())]
    #[inline]
    async fn muted_watcher(
        receiver: Rc<Receiver>,
        _: Rc<ReceiverState>,
        muted: Guarded<bool>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        receiver.set_muted(*muted);

        Ok(())
    }

    /// Watcher for the [`ReceiverState::enabled_individual`] update.
    ///
    /// Calls [`Receiver::set    #[inline]_enabled_individual_state`] with a new
    /// value.
    #[watch(self.state().enabled_individual.subscribe())]
    #[inline]
    async fn enabled_individual_watcher(
        receiver: Rc<Receiver>,
        _: Rc<ReceiverState>,
        enabled_individual: Guarded<bool>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        receiver.set_enabled_individual_state(*enabled_individual);

        Ok(())
    }

    /// Watcher for the [`ReceiverState::enabled_general`] update.
    ///
    /// Calls [`Receiver::set_enabled_general_state`] with a new value.
    #[watch(self.state().enabled_general.subscribe())]
    #[inline]
    async fn enabled_general_watcher(
        receiver: Rc<Receiver>,
        _: Rc<ReceiverState>,
        enabled_general: Guarded<bool>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        receiver.set_enabled_general_state(*enabled_general);

        Ok(())
    }
}
