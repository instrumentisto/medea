//! Implementation of [`Component`] for `MediaTrack` with a `Recv` direction.

use std::rc::Rc;

use futures::StreamExt as _;
use futures::{channel::mpsc, future::LocalBoxFuture};
use medea_client_api_proto::{
    state as proto_state, MediaSourceKind, MediaType, MemberId, TrackId,
    TrackPatchEvent,
};
use medea_macro::{watch, watchers};
use medea_reactive::{
    Guarded, ObservableCell, ProgressableCell, RecheckableFutureExt,
};

use crate::{
    media::RecvConstraints,
    peer::{
        component::SyncState,
        media::{
            media_exchange_state, InTransition, MediaStateControllable, Result,
        },
        MediaConnections, MediaExchangeState, MediaExchangeStateController,
        MuteState, MuteStateController, TrackEvent, TransceiverDirection,
        TransceiverSide,
    },
    utils::{component, AsProtoState, SynchronizableState, Updatable},
    MediaKind,
};

use super::Receiver;

/// Component responsible for the [`Receiver`] enabling/disabling and
/// muting/unmuting.
pub type Component = component::Component<State, Receiver>;

impl Component {
    /// Returns new [`Component`] with a provided [`State`].
    #[inline]
    pub fn new(
        state: Rc<State>,
        media_connections: &MediaConnections,
        track_events_sender: mpsc::UnboundedSender<TrackEvent>,
        recv_constraints: &RecvConstraints,
    ) -> Self {
        let enabled = match &state.media_type {
            MediaType::Audio(_) => recv_constraints.is_audio_enabled(),
            MediaType::Video(_) => recv_constraints.is_video_enabled(),
        };

        state.media_exchange_state.transition_to(enabled.into());

        let recv = Receiver::new(
            media_connections,
            state.id,
            state.media_type().clone().into(),
            state.sender_id().clone(),
            state.mid().clone(),
            state.enabled_general(),
            state.enabled_individual(),
            track_events_sender,
        );

        spawn_component!(Component, state, Rc::new(recv))
    }
}

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

    media_exchange_state: Rc<MediaExchangeStateController>,

    general_media_exchange_state: ObservableCell<media_exchange_state::Stable>,

    sync_state: ObservableCell<SyncState>,
}

impl AsProtoState for State {
    type Output = proto_state::Receiver;

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
    type Input = proto_state::Receiver;

    fn from_proto(input: Self::Input) -> Self {
        Self {
            id: input.id,
            mid: input.mid,
            media_type: input.media_type,
            sender_id: input.sender_id,
            media_exchange_state: MediaExchangeStateController::new(
                input.enabled_individual.into(),
            ),
            general_media_exchange_state: ObservableCell::new(
                input.enabled_general.into(),
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
            futures::future::join_all(vec![self
                .media_exchange_state
                .when_stabilized()])
            .map(|_| ()),
        )
    }

    fn when_updated(&self) -> Box<dyn RecheckableFutureExt<Output = ()>> {
        Box::new(medea_reactive::join_all(vec![
            self.media_exchange_state.when_processed()
        ]))
    }
}

impl From<&State> for proto_state::Receiver {
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

impl From<proto_state::Receiver> for State {
    fn from(from: proto_state::Receiver) -> Self {
        Self {
            id: from.id,
            mid: from.mid,
            media_type: from.media_type,
            sender_id: from.sender_id,
            media_exchange_state: MediaExchangeStateController::new(
                from.enabled_individual.into(),
            ),
            general_media_exchange_state: ObservableCell::new(
                from.enabled_general.into(),
            ),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
    }
}

impl State {
    /// Returns [`State`] with a provided data.
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
            media_exchange_state: MediaExchangeStateController::new(
                true.into(),
            ),
            general_media_exchange_state: ObservableCell::new(true.into()),
            sync_state: ObservableCell::new(SyncState::Synced),
        }
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

    /// Returns current [`MemberId`] of the `Member` from which this
    /// [`State`] should receive media data.
    #[inline]
    pub fn sender_id(&self) -> &MemberId {
        &self.sender_id
    }

    /// Returns current individual media exchange state of this
    /// [`State`].
    #[inline]
    pub fn enabled_individual(&self) -> bool {
        self.media_exchange_state.enabled()
    }

    /// Returns current general media exchange state of this [`State`].
    #[inline]
    pub fn enabled_general(&self) -> bool {
        self.general_media_exchange_state.get() == media_exchange_state::Stable::Enabled
    }

    /// Updates this [`State`] with a provided [`TrackPatchEvent`].
    pub fn update(&self, track_patch: &TrackPatchEvent) {
        if self.id != track_patch.id {
            return;
        }
        if let Some(enabled_general) = track_patch.enabled_general {
            self.general_media_exchange_state.set(enabled_general.into());
        }
        if let Some(enabled_individual) = track_patch.enabled_individual {
            self.media_exchange_state.update(enabled_individual.into());
        }
    }

    /// Returns [`Future`] which will be resolved when [`State`] update
    /// will be applied on [`Receiver`].
    ///
    /// [`Future`]: std::future::Future
    pub fn when_updated(&self) -> impl RecheckableFutureExt<Output = ()> {
        todo!("General media exchange state");
        medea_reactive::join_all(vec![
            self.media_exchange_state.when_processed()
        ])
    }

    pub fn connection_lost(&self) {
        self.sync_state.set(SyncState::Desynced);
    }

    pub fn connection_recovered(&self) {
        self.sync_state.set(SyncState::Syncing);
    }
}

#[watchers]
impl Component {
    #[watch(self.state().general_media_exchange_state.subscribe())]
    async fn general_media_exchange_state_watcher(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        state: media_exchange_state::Stable,
    ) -> Result<()> {
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

    #[watch(self.state().media_exchange_state.subscribe_stable())]
    async fn stable_media_exchange_state_watcher(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        state: media_exchange_state::Stable,
    ) -> Result<()> {
        receiver
            .enabled_individual
            .set(state == media_exchange_state::Stable::Enabled);

        Ok(())
    }

    #[watch(self.state().media_exchange_state.subscribe_transition())]
    async fn transition_media_exchange_state_watcher(
        receiver: Rc<Receiver>,
        _: Rc<State>,
        state: media_exchange_state::Transition,
    ) -> Result<()> {
        receiver.send_media_exchange_state_intention(state);

        Ok(())
    }

    #[watch(self.state().sync_state.subscribe().skip(1))]
    async fn sync_state_watcher(
        receiver: Rc<Receiver>,
        state: Rc<State>,
        sync_state: SyncState,
    ) -> Result<()> {
        if let SyncState::Synced = sync_state {
            if let MediaExchangeState::Transition(transition) =
            state.media_exchange_state.state()
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
        Rc::clone(&self.media_exchange_state)
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

    fn mid(&self) -> Option<String> {
        // if self.mid.borrow().is_none() && self.transceiver.borrow().is_some()
        // {     if let Some(transceiver) =
        //     self.transceiver.borrow().as_ref().cloned()
        //     {
        //         self.mid.replace(Some(transceiver.mid()?));
        //     }
        // }
        // self.mid.borrow().clone()
        self.mid.clone()
    }

    fn is_transitable(&self) -> bool {
        true
    }
}
