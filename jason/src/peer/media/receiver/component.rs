//! [`Component`] for `MediaTrack` with a `Recv` direction.

use std::rc::Rc;

use futures::{future::LocalBoxFuture, StreamExt as _};
use medea_client_api_proto as proto;
use medea_client_api_proto::{
    MediaSourceKind, MediaType, MemberId, TrackId, TrackPatchEvent,
};
use medea_macro::watchers;
use medea_reactive::{AllProcessed, Guarded, ObservableCell, ProgressableCell};

use crate::{
    media::LocalTracksConstraints,
    peer::{
        component::SyncState,
        media::{
            transitable_state::media_exchange_state, InTransition, Result,
        },
        MediaExchangeState, MediaExchangeStateController,
        MediaStateControllable, MuteStateController, TransceiverDirection,
        TransceiverSide,
    },
    utils::{component, AsProtoState, SynchronizableState, Updatable},
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

    /// Mid of this [`ReceiverComponent`].
    mid: Option<String>,

    /// [`MediaType`] of this [`ReceiverComponent`].
    media_type: MediaType,

    /// `Member`s which sends media to this [`ReceiverComponent`].
    sender_id: MemberId,

    enabled_individual: Rc<MediaExchangeStateController>,
    enabled_general: ProgressableCell<media_exchange_state::Stable>,

    sync_state: ObservableCell<SyncState>,
}

impl AsProtoState for State {
    type Output = proto::state::Receiver;

    fn as_proto(&self) -> Self::Output {
        Self::Output {
            id: self.id,
            mid: self.mid.clone(),
            media_type: self.media_type.clone(),
            sender_id: self.sender_id.clone(),
            enabled_individual: self.enabled_individual(),
            enabled_general: self.enabled_general(),
            muted: false,
        }
    }
}

impl SynchronizableState for State {
    type Input = proto::state::Receiver;

    fn from_proto(input: Self::Input, _: &LocalTracksConstraints) -> Self {
        Self {
            id: input.id,
            mid: input.mid,
            media_type: input.media_type,
            sender_id: input.sender_id,
            enabled_individual: MediaExchangeStateController::new(
                input.enabled_individual.into(),
            ),
            enabled_general: ProgressableCell::new(
                input.enabled_general.into(),
            ),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
    }

    fn apply(&self, input: Self::Input, _: &LocalTracksConstraints) {
        let new_media_exchange_state =
            media_exchange_state::Stable::from(input.enabled_individual);
        let current_media_exchange_state = match self.enabled_individual.state()
        {
            MediaExchangeState::Transition(transition) => {
                transition.into_inner()
            }
            MediaExchangeState::Stable(stable) => stable,
        };
        if current_media_exchange_state != new_media_exchange_state {
            self.enabled_individual.update(new_media_exchange_state);
        }

        let new_general_media_exchange_state =
            media_exchange_state::Stable::from(input.enabled_general);
        self.enabled_general.set(new_general_media_exchange_state);

        self.sync_state.set(SyncState::Synced);
    }
}

impl Updatable for State {
    fn when_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        use futures::FutureExt as _;
        Box::pin(
            futures::future::join_all(vec![self
                .enabled_individual
                .when_stabilized()])
            .map(|_| ()),
        )
    }

    fn when_updated(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![self
            .enabled_individual
            .when_processed()
            .into()])
    }
}

impl From<&State> for proto::state::Receiver {
    fn from(from: &State) -> Self {
        Self {
            id: from.id,
            mid: from.mid.clone(),
            media_type: from.media_type.clone(),
            sender_id: from.sender_id.clone(),
            enabled_individual: from.enabled_individual(),
            enabled_general: from.enabled_general(),
            muted: false,
        }
    }
}

impl From<proto::state::Receiver> for State {
    fn from(from: proto::state::Receiver) -> Self {
        Self {
            id: from.id,
            mid: from.mid,
            media_type: from.media_type,
            sender_id: from.sender_id,
            enabled_individual: MediaExchangeStateController::new(
                from.enabled_individual.into(),
            ),
            enabled_general: ProgressableCell::new(from.enabled_general.into()),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
    }
}

impl State {
    /// Returns [`State`] with a provided data.
    #[inline]
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
            sync_state: ObservableCell::new(SyncState::Synced),
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

    /// Notifies [`State`] about RPC connection loss.
    pub fn connection_lost(&self) {
        self.sync_state.set(SyncState::Desynced);
    }

    /// Notifies [`State`] about RPC connection restore.
    pub fn connection_recovered(&self) {
        self.sync_state.set(SyncState::Syncing);
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
    async fn enabled_individual_stable_state_changed(
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
    /// Sends [`TrackEvent::MediaExchangeIntention`] with a provided
    /// [`media_exchange_state`].
    #[watch(self.enabled_individual.subscribe_transition())]
    async fn enabled_individual_transition_started(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        state: media_exchange_state::Transition,
    ) -> Result<()> {
        receiver.send_media_exchange_state_intention(state);

        Ok(())
    }

    #[watch(self.sync_state.subscribe().skip(1))]
    async fn sync_state_watcher(
        receiver: Rc<Receiver>,
        state: Rc<State>,
        sync_state: SyncState,
    ) -> Result<()> {
        if let SyncState::Synced = sync_state {
            if let MediaExchangeState::Transition(transition) =
                state.enabled_individual.state()
            {
                receiver.send_media_exchange_state_intention(transition);
            }
        }

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
