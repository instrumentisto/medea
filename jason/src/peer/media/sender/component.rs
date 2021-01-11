//! Implementation of [`Component`] for `MediaTrack` with a `Send` direction.

use std::{cell::Cell, rc::Rc};

use futures::{channel::mpsc, StreamExt as _};
use medea_client_api_proto::{
    state as proto_state, MediaSourceKind, MediaType, MemberId, TrackId,
    TrackPatchEvent,
};
use medea_macro::{watch, watchers};
use medea_reactive::{
    Guarded, ObservableCell, ProgressableCell, RecheckableFutureExt,
};

use crate::{
    media::LocalTracksConstraints,
    peer::{
        media::{media_exchange_state, mute_state, Result},
        MediaConnections, MediaConnectionsError,
    },
    utils::{component, AsProtoState, SynchronizableState, Updatable},
    MediaKind,
};

use super::{Builder, Sender};
use crate::peer::{
    component::SyncState,
    conn::RTCPeerConnectionError::PeerConnectionEventBindFailed,
    media::InTransition, MediaExchangeState, MediaExchangeStateController,
    MediaState, MediaStateControllable, MuteState, MuteStateController,
    PeerEvent, TrackEvent, TransceiverDirection, TransceiverSide,
};
use futures::future::LocalBoxFuture;

/// Component responsible for the [`Sender`] enabling/disabling and
/// muting/unmuting.
pub type Component = component::Component<State, Sender>;

impl Component {
    /// Returns new [`Component`] with a provided [`State`].
    ///
    /// # Errors
    ///
    /// Returns [`MediaConnectionsError`] if [`Sender`] build fails.
    #[inline]
    pub fn new(
        state: Rc<State>,
        media_connections: &MediaConnections,
        send_constraints: LocalTracksConstraints,
        track_events_sender: mpsc::UnboundedSender<TrackEvent>,
    ) -> Result<Self> {
        let media_exchange_state = media_exchange_state::Stable::from(
            send_constraints.enabled(state.media_type()),
        );
        let mute_state = mute_state::Stable::from(
            send_constraints.muted(state.media_type()),
        );

        state.mute_state_controller().transition_to(mute_state);
        state
            .media_exchange_state_controller()
            .transition_to(media_exchange_state);

        let sndr = Builder {
            media_connections: &media_connections,
            track_id: state.id,
            caps: state.media_type().clone().into(),
            mute_state: mute_state::Stable::from(state.is_muted()),
            mid: state.mid().clone(),
            media_exchange_state: media_exchange_state::Stable::from(
                !state.is_enabled_individual(),
            ),
            required: state.media_type().required(),
            send_constraints,
            track_events_sender,
        }
        .build()
        .map_err(tracerr::map_from_and_wrap!())?;

        Ok(spawn_component!(Component, state, sndr))
    }
}

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

    /// Flag which indicates that local `MediaStream` update needed for this
    /// [`SenderComponent`].
    need_local_stream_update: Cell<bool>,

    media_exchange_state: Rc<MediaExchangeStateController>,

    mute_state: Rc<MuteStateController>,

    general_media_exchange_state: ObservableCell<media_exchange_state::Stable>,

    sync_state: ObservableCell<SyncState>,
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

    fn mid(&self) -> Option<String> {
        self.mid.clone()
    }

    fn is_transitable(&self) -> bool {
        // TODO (evdokimovs): Temp code. Rewrite it using is_constraints.
        match &self.media_type {
            MediaType::Video(video) => match &video.source_kind {
                MediaSourceKind::Display => false,
                MediaSourceKind::Device => true,
            },
            _ => true,
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
        if self.media_type.required() {
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
            general_media_exchange_state: ObservableCell::new(
                media_exchange_state::Stable::from(input.enabled_general),
            ),
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

    fn when_updated(&self) -> Box<dyn RecheckableFutureExt<Output = ()>> {
        Box::new(medea_reactive::join_all(vec![
            Box::new(self.media_exchange_state.when_processed()) as Box<dyn RecheckableFutureExt<Output = ()>>,
            Box::new(self.mute_state.when_processed()),
        ]))
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
            general_media_exchange_state: ObservableCell::new(
                media_exchange_state::Stable::from(from.enabled_general),
            ),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
    }
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
            need_local_stream_update: Cell::new(false),
            media_exchange_state: MediaExchangeStateController::new(
                media_exchange_state::Stable::from(true),
            ),
            general_media_exchange_state: ObservableCell::new(
                media_exchange_state::Stable::from(true),
            ),
            mute_state: MuteStateController::new(mute_state::Stable::from(
                false,
            )),
            sync_state: ObservableCell::new(SyncState::Synced),
        })
    }

    pub fn connection_lost(&self) {
        self.sync_state.set(SyncState::Desynced);
    }

    pub fn connection_recovered(&self) {
        self.sync_state.set(SyncState::Syncing);
    }

    pub fn enabled(&self) -> bool {
        self.media_exchange_state.enabled()
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
        self.media_exchange_state.enabled()
    }

    /// Returns current general media exchange state of this [`State`].
    #[inline]
    pub fn is_enabled_general(&self) -> bool {
        self.general_media_exchange_state.get()
            == media_exchange_state::Stable::Enabled
    }

    /// Returns current mute state of this [`State`].
    #[inline]
    pub fn is_muted(&self) -> bool {
        self.mute_state.muted()
    }

    /// Updates this [`State`] with a provided [`TrackPatchEvent`].
    pub fn update(&self, track_patch: &TrackPatchEvent) {
        if track_patch.id != self.id {
            return;
        }
        if let Some(enabled_general) = track_patch.enabled_general {
            self.general_media_exchange_state
                .set(media_exchange_state::Stable::from(enabled_general));
            // self.enabled_general.set(enabled_general);
        }
        if let Some(enabled_individual) = track_patch.enabled_individual {
            self.media_exchange_state
                .update(media_exchange_state::Stable::from(enabled_individual));
            // self.enabled_individual.set(enabled_individual);
        }
        if let Some(muted) = track_patch.muted {
            self.mute_state.update(mute_state::Stable::from(muted));
            // self.muted.set(muted);
        }
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
    #[watch(self.state().media_exchange_state.subscribe_transition())]
    async fn individual_media_exchange_state_transition_watcher(
        sender: Rc<Sender>,
        state: Rc<State>,
        new_state: media_exchange_state::Transition,
    ) -> Result<()> {
        sender.send_media_exchange_state_intention(new_state);

        Ok(())
    }

    #[watch(self.state().mute_state.subscribe_transition())]
    async fn mute_state_transition_watcher(
        sender: Rc<Sender>,
        state: Rc<State>,
        new_state: mute_state::Transition,
    ) -> Result<()> {
        sender.send_mute_state_intention(new_state);

        Ok(())
    }

    #[watch(self.state().general_media_exchange_state.subscribe())]
    async fn general_media_exchange_state_watcher(
        sender: Rc<Sender>,
        _: Rc<State>,
        new_state: media_exchange_state::Stable,
    ) -> Result<()> {
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

    #[watch(self.state().media_exchange_state.subscribe_stable())]
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

    #[watch(self.state().mute_state.subscribe_stable())]
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

    #[watch(self.state().sync_state.subscribe().skip(1))]
    async fn sync_state_watcher(
        sender: Rc<Sender>,
        state: Rc<State>,
        sync_state: SyncState,
    ) -> Result<()> {
        if let SyncState::Synced = sync_state {
            if let MediaExchangeState::Transition(transition) =
                state.media_exchange_state.state()
            {
                sender.send_media_exchange_state_intention(transition);
            }
            if let MuteState::Transition(transition) = state.mute_state.state()
            {
                sender.send_mute_state_intention(transition);
            }
        }

        Ok(())
    }
}
