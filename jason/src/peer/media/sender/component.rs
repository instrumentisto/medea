//! [`Component`] for `MediaTrack` with a `Send` direction.

use std::{cell::Cell, rc::Rc};

use futures::{future, future::LocalBoxFuture, FutureExt as _};
use medea_client_api_proto::{
    state as proto_state, MediaSourceKind, MediaType, MemberId, TrackId,
    TrackPatchEvent,
};
use medea_macro::watchers;
use medea_reactive::{AllProcessed, Guarded, ProgressableCell, ObservableCell};

use crate::{
    media::{LocalTracksConstraints, TrackConstraints, VideoSource},
    peer::{
        media::{media_exchange_state, mute_state, Result},
        MediaConnectionsError, MediaExchangeStateController, MediaState,
        MediaStateControllable, MuteStateController, TransceiverDirection,
        TransceiverSide,
    },
    utils::{component, AsProtoState, SynchronizableState, Updatable},
    MediaKind,
};
use crate::peer::component::SyncState;

use super::Sender;
use crate::peer::{MediaExchangeState, MuteState};
use crate::peer::media::InTransition;

/// Component responsible for the [`Sender`] enabling/disabling and
/// muting/unmuting.
pub type Component = component::Component<State, Sender>;

/// State of the [`Component`].
#[derive(Debug)]
pub struct State {
    id: TrackId,

    /// Mid of this [`SenderComponent`].
    mid: Option<String>,

    /// [`MediaType`] of this [`SenderComponent`].
    media_type: MediaType,

    /// All `Member`s which are receives media from this [`SenderComponent`].
    receivers: Vec<MemberId>,
    need_local_stream_update: Cell<bool>,
    media_exchange_state: Rc<MediaExchangeStateController>,
    mute_state: Rc<MuteStateController>,
    general_media_exchange_state:
    ProgressableCell<media_exchange_state::Stable>,
    send_constraints: LocalTracksConstraints,
    sync_state: ObservableCell<SyncState>,
}

impl AsProtoState for State {
    type Output = proto_state::Sender;

    fn as_proto(&self) -> Self::Output {
        Self::Output {
            id: self.id,
            mid: self.mid.clone(),
            media_type: self.media_type.clone(),
            receivers: self.receivers.clone(),
            enabled_individual: self.media_exchange_state.enabled(),
            enabled_general: self.general_media_exchange_state.get()
                == media_exchange_state::Stable::Enabled,
            muted: self.mute_state.muted(),
        }
    }
}

impl SynchronizableState for State {
    type Input = proto_state::Sender;

    fn from_proto(input: Self::Input) -> Self {
        todo!("Send constraints!!");
        Self {
            id: input.id,
            mid: input.mid,
            media_type: input.media_type,
            receivers: input.receivers,
            need_local_stream_update: Cell::new(false),
            mute_state: MuteStateController::new(mute_state::Stable::from(
                input.muted,
            )),
            media_exchange_state: MediaExchangeStateController::new(
                media_exchange_state::Stable::from(input.enabled_individual),
            ),
            general_media_exchange_state: ProgressableCell::new(
                media_exchange_state::Stable::from(input.enabled_general),
            ),
            send_constraints: Default::default(),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
    }

    fn apply(&self, input: Self::Input) {
        let new_media_exchange_state =
            media_exchange_state::Stable::from(input.enabled_individual);
        let current_media_exchange_state =
            match self.media_exchange_state.state() {
                MediaExchangeState::Transition(transition) => {
                    transition.into_inner()
                }
                MediaExchangeState::Stable(stable) => stable,
            };
        if current_media_exchange_state != new_media_exchange_state {
            self.media_exchange_state.update(new_media_exchange_state);
        }

        let new_mute_state = mute_state::Stable::from(input.muted);
        let current_mute_state = match self.mute_state.state() {
            MuteState::Stable(stable) => stable,
            MuteState::Transition(transition) => transition.into_inner(),
        };
        if current_mute_state != new_mute_state {
            self.mute_state.update(new_mute_state);
        }

        let new_general_media_exchange_state =
            media_exchange_state::Stable::from(input.enabled_general);
        self.general_media_exchange_state
            .set(new_general_media_exchange_state);

        self.sync_state.set(SyncState::Synced);
    }
}

impl Updatable for State {
    fn when_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        use futures::FutureExt as _;
        Box::pin(
            futures::future::join_all(vec![
                self.media_exchange_state.when_stabilized(),
                self.mute_state.when_stabilized(),
            ])
            .map(|_| ()),
        )
    }

    fn when_updated(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![
           self.media_exchange_state.when_processed().into(),
            self.mute_state.when_processed().into(),
        ])
    }
}

impl From<&State> for proto_state::Sender {
    fn from(state: &State) -> Self {
        Self {
            id: state.id,
            mid: state.mid.clone(),
            media_type: state.media_type.clone(),
            receivers: state.receivers.clone(),
            enabled_individual: state.media_exchange_state.enabled(),
            enabled_general: state.general_media_exchange_state.get()
                == media_exchange_state::Stable::Enabled,
            muted: state.mute_state.muted(),
        }
    }
}

impl From<proto_state::Sender> for State {
    fn from(from: proto_state::Sender) -> Self {
        todo!("send_constraints!!");
        Self {
            id: from.id,
            mid: from.mid,
            media_type: from.media_type,
            receivers: from.receivers,
            need_local_stream_update: Cell::new(false),
            mute_state: MuteStateController::new(mute_state::Stable::from(
                from.muted,
            )),
            media_exchange_state: MediaExchangeStateController::new(
                media_exchange_state::Stable::from(from.enabled_individual),
            ),
            general_media_exchange_state: ProgressableCell::new(
                media_exchange_state::Stable::from(from.enabled_general),
            ),
            send_constraints: Default::default(),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
    }
}

impl State {
    /// Creates new [`State`] with the provided data.
    ///
    /// # Errors
    ///
    /// Returns [`MediaConnectionsError::CannotDisableRequiredSender`] if this
    /// [`Sender`] cannot be disabled.
    pub fn new(
        id: TrackId,
        mid: Option<String>,
        media_type: MediaType,
        receivers: Vec<MemberId>,
        send_constraints: LocalTracksConstraints,
    ) -> Result<Self> {
        Ok(Self {
            id,
            mid,
            media_type,
            receivers,
            need_local_stream_update: Cell::new(false),
            media_exchange_state: MediaExchangeStateController::new(
                media_exchange_state::Stable::from(true),
            ),
            general_media_exchange_state: ProgressableCell::new(
                media_exchange_state::Stable::from(true),
            ),
            mute_state: MuteStateController::new(mute_state::Stable::from(
                false,
            )),
            sync_state: ObservableCell::new(SyncState::Synced),
            send_constraints,
        })
    }

    pub fn connection_lost(&self) {
        self.sync_state.set(SyncState::Desynced);
    }

    pub fn connection_recovered(&self) {
        self.sync_state.set(SyncState::Syncing);
    }

    /// Indicates whether [`Sender`]'s media exchange state is in
    /// [`media_exchange_state::Stable::Enabled`].
    #[inline]
    pub fn enabled(&self) -> bool {
        self.media_exchange_state.enabled()
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

    /// Returns current [`MemberId`]s of the `Member`s that this [`State`]
    /// should send media data to.
    #[inline]
    #[must_use]
    pub fn receivers(&self) -> &Vec<MemberId> {
        &self.receivers
    }

    /// Returns current individual media exchange state of this [`State`].
    #[inline]
    #[must_use]
    pub fn is_enabled_individual(&self) -> bool {
        self.media_exchange_state.enabled()
    }

    /// Returns current general media exchange state of this [`State`].
    #[inline]
    #[must_use]
    pub fn is_enabled_general(&self) -> bool {
        self.general_media_exchange_state.get()
            == media_exchange_state::Stable::Enabled
    }

    /// Returns current mute state of this [`State`].
    #[inline]
    #[must_use]
    pub fn is_muted(&self) -> bool {
        self.mute_state.muted()
    }

    /// Updates this [`State`] with the provided [`TrackPatchEvent`].
    pub fn update(&self, track_patch: &TrackPatchEvent) {
        if track_patch.id != self.id {
            return;
        }
        if let Some(enabled_general) = track_patch.enabled_general {
            self.general_media_exchange_state
                .set(media_exchange_state::Stable::from(enabled_general));
        }
        if let Some(enabled_individual) = track_patch.enabled_individual {
            self.media_exchange_state
                .update(media_exchange_state::Stable::from(enabled_individual));
        }
        if let Some(muted) = track_patch.muted {
            self.mute_state.update(mute_state::Stable::from(muted));
        }
    }

    /// Returns [`Future`] resolving when [`State`] update will be applied onto
    /// [`Sender`].
    ///
    /// [`Future`]: std::future::Future
    pub fn when_updated(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![
            self.media_exchange_state.when_processed().into(),
            self.mute_state.when_processed().into(),
            self.general_media_exchange_state
                .when_all_processed()
                .into(),
        ])
    }

    /// Returns [`Future`] which will be resolved when [`media_exchange_state`]
    /// and [`mute_state`] will be stabilized.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_stabilized(&self) -> LocalBoxFuture<'static, ()> {
        Box::pin(
            future::join_all(vec![
                self.media_exchange_state.when_stabilized(),
                self.mute_state.when_stabilized(),
            ])
            .map(|_| ()),
        )
    }

    /// Indicates whether local `MediaStream` update needed for this [`State`].
    #[inline]
    #[must_use]
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
    #[must_use]
    pub fn media_kind(&self) -> MediaKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaKind::Audio,
            MediaType::Video(_) => MediaKind::Video,
        }
    }

    /// Returns [`MediaSourceKind`] of this [`State`].
    #[inline]
    #[must_use]
    pub fn media_source(&self) -> MediaSourceKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaSourceKind::Device,
            MediaType::Video(video) => video.source_kind,
        }
    }
}

#[watchers]
impl Component {
    /// Watcher for [`MediaExchangeState::Transition`] update.
    ///
    /// Sends new intention by [`Receiver::send_media_exchange_state_intention`]
    /// call.
    #[watch(self.media_exchange_state.subscribe_transition())]
    async fn individual_media_exchange_state_transition_watcher(
        sender: Rc<Sender>,
        _: Rc<State>,
        new_state: media_exchange_state::Transition,
    ) -> Result<()> {
        sender.send_media_exchange_state_intention(new_state);

        Ok(())
    }

    /// Watcher for [`MuteState::Transition`] update.
    ///
    /// Sends new intention by [`Receiver::send_mute_state_intention`] call.
    #[watch(self.mute_state.subscribe_transition())]
    async fn mute_state_transition_watcher(
        sender: Rc<Sender>,
        _: Rc<State>,
        new_state: mute_state::Transition,
    ) -> Result<()> {
        sender.send_mute_state_intention(new_state);

        Ok(())
    }

    /// Watcher for the [`State::general_media_exchange_state`] update.
    ///
    /// Updates [`Sender`]'s general media exchange state. Adds or removes
    /// [`TransceiverDirection::SEND`] from the [`Transceiver`] of the
    /// [`Receiver`].
    #[watch(self.general_media_exchange_state.subscribe())]
    async fn general_media_exchange_state_watcher(
        sender: Rc<Sender>,
        _: Rc<State>,
        new_state: Guarded<media_exchange_state::Stable>,
    ) -> Result<()> {
        let (new_state, _guard) = new_state.into_parts();

        sender.set_enabled_general(
            new_state == media_exchange_state::Stable::Enabled,
        );
        match new_state {
            media_exchange_state::Stable::Enabled => {
                if sender.enabled_in_cons() {
                    sender
                        .transceiver
                        .add_direction(TransceiverDirection::SEND);
                }
            }
            media_exchange_state::Stable::Disabled => {
                sender.transceiver.sub_direction(TransceiverDirection::SEND);
            }
        }

        Ok(())
    }

    /// Watcher for [`MediaExchangeState::Stable`] update.
    ///
    /// Updates [`Receiver::enabled_individual`] to the new state.
    ///
    /// Removes `MediaTrack` from [`Transceiver`] if new state is
    /// [`media_exchange_state::Stable::Disabled`].
    ///
    /// Sets [`State::need_local_stream_update`] to the `true` if state is
    /// [`media_exchange_state::Stable::Enabled`].
    #[watch(self.media_exchange_state.subscribe_stable())]
    async fn individual_media_exchange_state_stable_watcher(
        sender: Rc<Sender>,
        state: Rc<State>,
        new_state: media_exchange_state::Stable,
    ) -> Result<()> {
        sender.set_enabled_individual(
            new_state == media_exchange_state::Stable::Enabled,
        );
        match new_state {
            media_exchange_state::Stable::Enabled => {
                state.need_local_stream_update.set(true);
            }
            media_exchange_state::Stable::Disabled => {
                sender.remove_track().await;
            }
        }

        Ok(())
    }

    /// Watcher for [`MuteState::Stable`] update.
    ///
    /// Updates [`Sender`]'s mute state.
    ///
    /// Updates [`Sender`]'s [`Transceiver`] `MediaTrack.enabled` property.
    #[watch(self.mute_state.subscribe_stable())]
    async fn mute_state_stable_watcher(
        sender: Rc<Sender>,
        _: Rc<State>,
        new_state: mute_state::Stable,
    ) -> Result<()> {
        sender.set_muted(new_state == mute_state::Stable::Muted);
        match new_state {
            mute_state::Stable::Muted => {
                sender.transceiver.set_send_track_enabled(false);
            }
            mute_state::Stable::Unmuted => {
                sender.transceiver.set_send_track_enabled(true);
            }
        }

        Ok(())
    }
}

impl TransceiverSide for State {
    fn track_id(&self) -> TrackId {
        self.id
    }

    fn kind(&self) -> MediaKind {
        self.media_kind()
    }

    fn source_kind(&self) -> MediaSourceKind {
        self.media_source()
    }

    fn is_transitable(&self) -> bool {
        let caps = TrackConstraints::from(self.media_type.clone());
        match &caps {
            TrackConstraints::Video(VideoSource::Device(_)) => {
                self.send_constraints.inner().get_device_video().is_some()
            }
            TrackConstraints::Video(VideoSource::Display(_)) => {
                self.send_constraints.inner().get_display_video().is_some()
            }
            TrackConstraints::Audio(_) => true,
        }
    }
}

impl MediaStateControllable for State {
    fn media_exchange_state_controller(
        &self,
    ) -> Rc<MediaExchangeStateController> {
        Rc::clone(&self.media_exchange_state)
    }

    fn mute_state_controller(&self) -> Rc<MuteStateController> {
        Rc::clone(&self.mute_state)
    }

    fn media_state_transition_to(
        &self,
        desired_state: MediaState,
    ) -> Result<()> {
        if self.media_type.required()
            && matches!(
                desired_state,
                MediaState::Mute(mute_state::Stable::Muted)
                    | MediaState::MediaExchange(
                        media_exchange_state::Stable::Disabled
                    )
            )
        {
            Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
            ))
        } else {
            match desired_state {
                MediaState::MediaExchange(desired_state) => {
                    self.media_exchange_state_controller()
                        .transition_to(desired_state);
                }
                MediaState::Mute(desired_state) => {
                    self.mute_state_controller().transition_to(desired_state);
                }
            }
            Ok(())
        }
    }
}
