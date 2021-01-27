//! [`Component`] for `MediaTrack` with a `Send` direction.

use std::rc::Rc;

use futures::{future::LocalBoxFuture, StreamExt as _};
use medea_client_api_proto::{
    MediaSourceKind, MediaType, MemberId, TrackId, TrackPatchEvent,
};
use medea_macro::watchers;
use medea_reactive::{AllProcessed, Guarded, ObservableCell, ProgressableCell};
use tracerr::Traced;

use crate::{
    media::{LocalTracksConstraints, TrackConstraints, VideoSource},
    peer::{
        self,
        media::{media_exchange_state, mute_state, Result},
        MediaConnectionsError, MediaExchangeStateController, MediaState,
        MediaStateControllable, MuteStateController, PeerError, TrackEvent,
        TransceiverDirection, TransceiverSide,
    },
    utils::component,
    MediaKind,
};

use super::Sender;

/// State of the [`local::Track`] of the [`Sender`].
///
/// [`PartialEq`] implementation of this state ignores
/// [`LocalTrackState::Failed`] content.
#[derive(Debug, Clone)]
enum LocalTrackState {
    /// Indicates that [`Sender`] is new, or [`local::Track`] is set.
    Stable,

    /// Indicates that [`Sender`] needs a new [`local::Track`].
    NeedUpdate,

    /// Indicates that new [`local::Track`] getting is failed.
    ///
    /// Contains [`PeerError`] with which
    /// [getUserMedia()][1]/[getDisplayMedia()][2] request was failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    Failed(Traced<PeerError>),
}

impl PartialEq for LocalTrackState {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::NeedUpdate => matches!(other, Self::NeedUpdate),
            Self::Stable => matches!(other, Self::Stable),
            Self::Failed(_) => matches!(other, Self::Failed(_)),
        }
    }
}

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
    enabled_individual: Rc<MediaExchangeStateController>,
    mute_state: Rc<MuteStateController>,
    enabled_general: ProgressableCell<media_exchange_state::Stable>,
    send_constraints: LocalTracksConstraints,
    local_track_state: ObservableCell<LocalTrackState>,
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
            enabled_individual: MediaExchangeStateController::new(
                media_exchange_state::Stable::from(true),
            ),
            enabled_general: ProgressableCell::new(
                media_exchange_state::Stable::from(true),
            ),
            mute_state: MuteStateController::new(mute_state::Stable::from(
                false,
            )),
            send_constraints,
            local_track_state: ObservableCell::new(LocalTrackState::Stable),
        })
    }

    /// Indicates whether this [`Sender`]'s media exchange state is in
    /// [`media_exchange_state::Stable::Enabled`].
    #[inline]
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled_individual.enabled()
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
        self.enabled_individual.enabled()
    }

    /// Returns current general media exchange state of this [`State`].
    #[inline]
    #[must_use]
    pub fn is_enabled_general(&self) -> bool {
        self.enabled_general.get() == media_exchange_state::Stable::Enabled
    }

    /// Returns current mute state of this [`State`].
    #[inline]
    #[must_use]
    pub fn is_muted(&self) -> bool {
        self.mute_state.muted()
    }

    /// Returns [`Future`] which will be resolved once
    /// [getUserMedia()][1]/[getDisplayMedia()][2] request for this [`State`] is
    /// resolved.
    ///
    /// [`Future`]: std::future::Future
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub fn local_stream_update_result(
        &self,
    ) -> LocalBoxFuture<'static, peer::Result<()>> {
        let mut local_track_state_rx = self.local_track_state.subscribe();
        Box::pin(async move {
            while let Some(s) = local_track_state_rx.next().await {
                match s {
                    LocalTrackState::Stable => return Ok(()),
                    LocalTrackState::Failed(err) => {
                        return Err(tracerr::new!(err))
                    }
                    LocalTrackState::NeedUpdate => (),
                }
            }

            Ok(())
        })
    }

    /// Updates this [`State`] with the provided [`TrackPatchEvent`].
    pub fn update(&self, track_patch: &TrackPatchEvent) {
        if track_patch.id != self.id {
            return;
        }
        if let Some(enabled_general) = track_patch.enabled_general {
            self.enabled_general
                .set(media_exchange_state::Stable::from(enabled_general));
        }
        if let Some(enabled_individual) = track_patch.enabled_individual {
            self.enabled_individual
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
            self.enabled_individual.when_processed().into(),
            self.mute_state.when_processed().into(),
            self.enabled_general.when_all_processed().into(),
        ])
    }

    /// Returns [`Future`] which will be resolved once [`media_exchange_state`]
    /// and [`mute_state`] are be stabilized.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_stabilized(&self) -> AllProcessed<'static> {
        medea_reactive::when_all_processed(vec![
            Rc::clone(&self.enabled_individual).when_stabilized().into(),
            Rc::clone(&self.mute_state).when_stabilized().into(),
        ])
    }

    /// Indicates whether local `MediaStream` update needed for this [`State`].
    #[inline]
    #[must_use]
    pub fn is_local_stream_update_needed(&self) -> bool {
        matches!(self.local_track_state.get(), LocalTrackState::NeedUpdate)
    }

    /// Transits [`State::local_track_state`] to a failed state.
    #[inline]
    pub fn failed_local_stream_update(&self, error: Traced<PeerError>) {
        self.local_track_state.set(LocalTrackState::Failed(error));
    }

    /// Transits [`State::local_track_state`] to a stable state.
    #[inline]
    pub fn local_stream_updated(&self) {
        self.local_track_state.set(LocalTrackState::Stable);
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
    /// Sends [`TrackEvent::MediaExchangeIntention`] with the provided
    /// [`media_exchange_state`].
    #[watch(self.enabled_individual.subscribe_transition())]
    async fn enabled_individual_transition_started(
        sender: Rc<Sender>,
        _: Rc<State>,
        new_state: media_exchange_state::Transition,
    ) -> Result<()> {
        match new_state {
            media_exchange_state::Transition::Enabling(_) => {
                sender
                    .track_events_sender
                    .unbounded_send(TrackEvent::MediaExchangeIntention {
                        id: sender.track_id,
                        enabled: true,
                    })
                    .ok();
            }
            media_exchange_state::Transition::Disabling(_) => {
                sender
                    .track_events_sender
                    .unbounded_send(TrackEvent::MediaExchangeIntention {
                        id: sender.track_id,
                        enabled: false,
                    })
                    .ok();
            }
        }

        Ok(())
    }

    /// Watcher for [`MuteState::Transition`] update.
    ///
    /// Sends [`TrackEvent::MuteUpdateIntention`] with the provided
    /// [`mute_state`].
    #[watch(self.mute_state.subscribe_transition())]
    async fn mute_state_transition_watcher(
        sender: Rc<Sender>,
        _: Rc<State>,
        new_state: mute_state::Transition,
    ) -> Result<()> {
        let _ = sender.track_events_sender.unbounded_send(
            TrackEvent::MuteUpdateIntention {
                id: sender.track_id,
                muted: matches!(new_state, mute_state::Transition::Muting(_)),
            },
        );

        Ok(())
    }

    /// Watcher for the [`State::enabled_general`] update.
    ///
    /// Updates [`Sender`]'s general media exchange state. Adds or removes
    /// [`TransceiverDirection::SEND`] from the [`Transceiver`] of the
    /// [`Receiver`].
    #[watch(self.enabled_general.subscribe())]
    async fn enabled_general_state_changed(
        sender: Rc<Sender>,
        _: Rc<State>,
        new_state: Guarded<media_exchange_state::Stable>,
    ) -> Result<()> {
        let (new_state, _guard) = new_state.into_parts();

        sender
            .enabled_general
            .set(new_state == media_exchange_state::Stable::Enabled);
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

    /// Watcher for the [`MediaExchangeState::Stable`] update.
    ///
    /// Updates [`Receiver::enabled_individual`] to the `new_state`.
    ///
    /// Removes `MediaTrack` from [`Transceiver`] if `new_state` is
    /// [`media_exchange_state::Stable::Disabled`].
    ///
    /// Sets [`State::need_local_stream_update`] to the `true` if `new_state` is
    /// [`media_exchange_state::Stable::Enabled`].
    #[watch(self.enabled_individual.subscribe_stable())]
    async fn enabled_individual_stable_state_changed(
        sender: Rc<Sender>,
        state: Rc<State>,
        new_state: media_exchange_state::Stable,
    ) -> Result<()> {
        sender
            .enabled_individual
            .set(new_state == media_exchange_state::Stable::Enabled);
        match new_state {
            media_exchange_state::Stable::Enabled => {
                state.local_track_state.set(LocalTrackState::NeedUpdate);
            }
            media_exchange_state::Stable::Disabled => {
                sender.remove_track().await;
            }
        }
        Ok(())
    }

    /// Watcher for the [`MuteState::Stable`] update.
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
        sender.muted.set(new_state == mute_state::Stable::Muted);
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
    #[inline]
    fn track_id(&self) -> TrackId {
        self.id
    }

    #[inline]
    fn kind(&self) -> MediaKind {
        self.media_kind()
    }

    #[inline]
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
    #[inline]
    fn media_exchange_state_controller(
        &self,
    ) -> Rc<MediaExchangeStateController> {
        Rc::clone(&self.enabled_individual)
    }

    #[inline]
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
