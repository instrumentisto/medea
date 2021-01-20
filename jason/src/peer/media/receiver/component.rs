//! [`Component`] for `MediaTrack` with a `Recv` direction.

use std::rc::Rc;

use futures::future::LocalBoxFuture;
use medea_client_api_proto::{
    MediaSourceKind, MediaType, MemberId, TrackId, TrackPatchEvent,
};
use medea_macro::watchers;
use medea_reactive::{AllProcessed, Guarded, ProgressableCell};

use crate::{
    peer::{
        media::{transitable_state::media_exchange_state, Result},
        MediaExchangeStateController, MediaStateControllable,
        MuteStateController, TransceiverDirection, TransceiverSide,
    },
    utils::component,
    MediaKind,
};

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
    enabled_individual: Rc<MediaExchangeStateController>,
    enabled_general: ProgressableCell<media_exchange_state::Stable>,
}

impl State {
    /// Returns [`State`] with a provided data.
    #[must_use]
    pub fn new(
        id: TrackId,
        mid: Option<String>,
        media_type: MediaType,
        sender: MemberId,
    ) -> Self {
        Self {
            id,
            mid,
            media_type,
            sender_id: sender,
            enabled_individual: MediaExchangeStateController::new(
                media_exchange_state::Stable::Enabled,
            ),
            enabled_general: ProgressableCell::new(
                media_exchange_state::Stable::Enabled,
            ),
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
        self.enabled_individual.enabled()
    }

    /// Returns current general media exchange state of this [`State`].
    #[inline]
    #[must_use]
    pub fn enabled_general(&self) -> bool {
        self.enabled_general.get() == media_exchange_state::Stable::Enabled
    }

    /// Updates this [`State`] with the provided [`TrackPatchEvent`].
    pub fn update(&self, track_patch: &TrackPatchEvent) {
        if self.id != track_patch.id {
            return;
        }
        if let Some(enabled_general) = track_patch.enabled_general {
            self.enabled_general.set(enabled_general.into());
        }
        if let Some(enabled_individual) = track_patch.enabled_individual {
            self.enabled_individual.update(enabled_individual.into());
        }
    }

    /// Returns [`Future`] resolving when [`State`] update will be applied onto
    /// [`Receiver`].
    ///
    /// [`Future`]: std::future::Future
    pub fn when_updated(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![
            self.enabled_individual.when_processed().into(),
            self.enabled_general.when_all_processed().into(),
        ])
    }

    /// Returns [`Future`] which will be resolved when [`media_exchange_state`]
    /// will be stabilized.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        self.enabled_individual.when_stabilized()
    }
}

#[watchers]
impl Component {
    /// Watcher for the [`State::general_media_exchange_state`] update.
    ///
    /// Updates [`Receiver`]'s general media exchange state. Adds or removes
    /// [`TransceiverDirection::RECV`] from the [`Transceiver`] of the
    /// [`Receiver`].
    #[watch(self.enabled_general.subscribe())]
    async fn general_media_exchange_state_changed(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        state: Guarded<media_exchange_state::Stable>,
    ) -> Result<()> {
        let (state, _guard) = state.into_parts();
        receiver
            .enabled_general
            .set(state == media_exchange_state::Stable::Enabled);
        match state {
            media_exchange_state::Stable::Disabled => {
                if let Some(track) = receiver.track.borrow().as_ref() {
                    track.set_enabled(false);
                }
                if let Some(trnscvr) = receiver.transceiver.borrow().as_ref() {
                    trnscvr.sub_direction(TransceiverDirection::RECV);
                }
            }
            media_exchange_state::Stable::Enabled => {
                if let Some(track) = receiver.track.borrow().as_ref() {
                    track.set_enabled(true);
                }
                if let Some(trnscvr) = receiver.transceiver.borrow().as_ref() {
                    trnscvr.add_direction(TransceiverDirection::RECV);
                }
            }
        }
        receiver.maybe_notify_track();

        Ok(())
    }

    /// Watcher for [`MediaExchangeState::Stable`] update.
    ///
    /// Updates [`Receiver::enabled_individual`] to the new state.
    #[watch(self.enabled_individual.subscribe_stable())]
    async fn stable_media_exchange_state_changed(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        state: media_exchange_state::Stable,
    ) -> Result<()> {
        receiver
            .enabled_individual
            .set(state == media_exchange_state::Stable::Enabled);

        Ok(())
    }

    /// Watcher for [`MediaExchangeState::Transition`] update.
    ///
    /// Sends new intention by [`Receiver::send_media_exchange_state_intention`]
    /// call.
    #[watch(self.enabled_individual.subscribe_transition())]
    async fn transition_media_exchange_state_changed(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        state: media_exchange_state::Transition,
    ) -> Result<()> {
        receiver.send_media_exchange_state_intention(state);

        Ok(())
    }
}

impl MediaStateControllable for State {
    fn media_exchange_state_controller(
        &self,
    ) -> Rc<MediaExchangeStateController> {
        Rc::clone(&self.enabled_individual)
    }

    fn mute_state_controller(&self) -> Rc<MuteStateController> {
        // Receivers can be muted, but currently they are muted directly by
        // server events.
        //
        // There is no point to provide external API for muting receivers, since
        // muting is pipelined after demuxing and decoding, so it wont reduce
        // incoming traffic or CPU usage. Therefore receivers muting do not
        // require MuteStateController's state management.
        //
        // Removing this unreachable! would require abstracting
        // MuteStateController to some trait and creating some dummy
        // implementation. Not worth it atm.
        unreachable!("Receivers muting is not implemented");
    }

    /// Stops only [`MediaExchangeStateController`]'s state transition timer.
    #[inline]
    fn stop_media_state_transition_timeout(&self) {
        self.media_exchange_state_controller()
            .stop_transition_timeout();
    }

    /// Resets only [`MediaExchangeStateController`]'s state transition timer.
    #[inline]
    fn reset_media_state_transition_timeout(&self) {
        self.media_exchange_state_controller()
            .reset_transition_timeout();
    }
}

impl TransceiverSide for State {
    fn track_id(&self) -> TrackId {
        self.id
    }

    fn kind(&self) -> MediaKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaKind::Audio,
            MediaType::Video(_) => MediaKind::Video,
        }
    }

    fn source_kind(&self) -> MediaSourceKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaSourceKind::Device,
            MediaType::Video(video) => video.source_kind,
        }
    }

    fn is_transitable(&self) -> bool {
        true
    }
}

#[cfg(feature = "mockable")]
impl State {
    /// Stabilizes [`MediaExchangeState`] of this [`State`].
    #[inline]
    pub fn stabilize(&self) {
        use crate::peer::media::InTransition as _;

        if let crate::peer::MediaExchangeState::Transition(transition) =
            self.enabled_individual.state()
        {
            self.enabled_individual.update(transition.intended());
            self.enabled_general.set(transition.intended());
        }
    }
}
