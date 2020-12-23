//! Implementation of the [`SenderComponent`].

use std::{cell::Cell, rc::Rc};

use futures::future::LocalBoxFuture;
use medea_client_api_proto::{
    state as proto_state, MediaSourceKind, MediaType, MemberId, TrackId,
    TrackPatchEvent,
};
use medea_macro::{watch, watchers};
use medea_reactive::{Guarded, ProgressableCell};
use tracerr::Traced;

use crate::{
    api::GlobalCtx,
    media::LocalTracksConstraints,
    peer::{MediaConnectionsError, Sender},
    utils::{AsProtoState, Component, SynchronizableState, Updatable},
    MediaKind,
};

/// Component responsible for the [`Sender`] enabling/disabling and
/// muting/unmuting.
pub type SenderComponent = Component<SenderState, Sender, GlobalCtx>;

/// State of the [`SenderComponent`].
#[derive(Debug)]
pub struct SenderState {
    /// ID of this [`SenderComponent`].
    id: TrackId,

    /// Mid of this [`SenderComponent`].
    mid: Option<String>,

    /// [`MediaType`] of this [`SenderComponent`].
    media_type: MediaType,

    /// All `Member`s which are receives media from this [`SenderComponent`].
    receivers: Vec<MemberId>,

    /// Flag which indicates that this [`SenderComponent`] is enabled on `Send`
    /// direction side.
    enabled_individual: ProgressableCell<bool>,

    /// Flag which indicates that this [`SenderComponent`] is enabled on `Send`
    /// __and__ `Recv` direction sides.
    enabled_general: ProgressableCell<bool>,

    /// Flag which indicates that this [`SenderComponent`] is muted.
    muted: ProgressableCell<bool>,

    /// Flag which indicates that local `MediaStream` update needed for this
    /// [`SenderComponent`].
    need_local_stream_update: Cell<bool>,
}

impl AsProtoState for SenderState {
    type Output = proto_state::Sender;

    fn as_proto(&self) -> Self::Output {
        Self::Output {
            id: self.id,
            mid: self.mid.clone(),
            media_type: self.media_type.clone(),
            receivers: self.receivers.clone(),
            enabled_individual: self.enabled_individual.get(),
            enabled_general: self.enabled_general.get(),
            muted: self.muted.get(),
        }
    }
}

impl SynchronizableState for SenderState {
    type Input = proto_state::Sender;

    fn from_proto(input: Self::Input) -> Self {
        Self {
            id: input.id,
            mid: input.mid,
            media_type: input.media_type,
            receivers: input.receivers,
            enabled_individual: ProgressableCell::new(input.enabled_individual),
            enabled_general: ProgressableCell::new(input.enabled_general),
            muted: ProgressableCell::new(input.muted),
            need_local_stream_update: Cell::new(false),
        }
    }

    fn apply(&self, input: Self::Input) {
        self.muted.set(input.muted);
        self.enabled_general.set(input.enabled_general);
        self.enabled_individual.set(input.enabled_individual);
    }
}

impl Updatable for SenderState {
    fn when_updated(&self) -> LocalBoxFuture<'static, ()> {
        let fut = futures::future::join_all(vec![
            self.enabled_general.when_all_processed(),
            self.enabled_individual.when_all_processed(),
            self.muted.when_all_processed(),
        ]);
        Box::pin(async move {
            fut.await;
        })
    }
}

impl From<&SenderState> for proto_state::Sender {
    fn from(state: &SenderState) -> Self {
        Self {
            id: state.id,
            mid: state.mid.clone(),
            media_type: state.media_type.clone(),
            receivers: state.receivers.clone(),
            enabled_individual: state.enabled_individual.get(),
            enabled_general: state.enabled_general.get(),
            muted: state.muted.get(),
        }
    }
}

impl From<proto_state::Sender> for SenderState {
    fn from(from: proto_state::Sender) -> Self {
        Self {
            id: from.id,
            mid: from.mid,
            media_type: from.media_type,
            receivers: from.receivers,
            enabled_individual: ProgressableCell::new(from.enabled_individual),
            enabled_general: ProgressableCell::new(from.enabled_general),
            muted: ProgressableCell::new(from.muted),
            need_local_stream_update: Cell::new(false),
        }
    }
}

impl SenderState {
    /// Creates new [`SenderState`] with a provided data.
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
    ) -> Result<Self, Traced<MediaConnectionsError>> {
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

    /// Returns [`TrackId`] of this [`SenderState`].
    #[inline]
    pub fn id(&self) -> TrackId {
        self.id
    }

    /// Returns current `mid` of this [`SenderState`].
    #[inline]
    pub fn mid(&self) -> &Option<String> {
        &self.mid
    }

    /// Returns current [`MediaType`] of this [`SenderState`].
    #[inline]
    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    /// Returns current [`MemberId`]s of the `Member`s to which this
    /// [`SenderState`] should send media data.
    #[inline]
    pub fn receivers(&self) -> &Vec<MemberId> {
        &self.receivers
    }

    /// Returns current individual media exchange state of this [`SenderState`].
    #[inline]
    pub fn is_enabled_individual(&self) -> bool {
        self.enabled_individual.get()
    }

    /// Returns current general media exchange state of this [`SenderState`].
    #[inline]
    pub fn is_enabled_general(&self) -> bool {
        self.enabled_general.get()
    }

    /// Returns current mute state of this [`SenderState`].
    #[inline]
    pub fn is_muted(&self) -> bool {
        self.muted.get()
    }

    /// Updates this [`SenderState`] with a provided [`TrackPatchEvent`].
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

    /// Returns [`Future`] which will be resolved when [`SenderState`] update
    /// will be applied on [`Sender`].
    ///
    /// [`Future`]: std::future::Future
    pub fn when_updated(&self) -> LocalBoxFuture<'static, ()> {
        let fut = futures::future::join_all(vec![
            self.enabled_general.when_all_processed(),
            self.enabled_individual.when_all_processed(),
            self.muted.when_all_processed(),
        ]);
        Box::pin(async move {
            fut.await;
        })
    }

    /// Returns `true` if local `MediaStream` update needed for this
    /// [`SenderState`].
    #[inline]
    pub fn is_local_stream_update_needed(&self) -> bool {
        self.need_local_stream_update.get()
    }

    /// Sets [`SenderState::need_local_stream_update`] to `false`.
    #[inline]
    pub fn local_stream_updated(&self) {
        self.need_local_stream_update.set(false);
    }

    /// Returns [`MediaKind`] of this [`SenderState`].
    #[inline]
    pub fn media_kind(&self) -> MediaKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaKind::Audio,
            MediaType::Video(_) => MediaKind::Video,
        }
    }

    /// Returns [`MediaSourceKind`] of this [`SenderState`].
    #[inline]
    pub fn media_source(&self) -> MediaSourceKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaSourceKind::Device,
            MediaType::Video(video) => video.source_kind,
        }
    }
}

#[watchers]
impl SenderComponent {
    /// Watcher for the [`SenderState::enabled_individual`] update.
    ///
    /// Calls [`Sender::set_enabled_individual`] with a new value.
    ///
    /// If new value is `true` then sets
    /// [`SenderState::need_local_stream_update`] flag to `true`, otherwise
    /// calls [`Sender::remove_track`].
    #[watch(self.state().enabled_individual.subscribe())]
    #[inline]
    async fn enabled_individual_watcher(
        ctx: Rc<Sender>,
        _: Rc<GlobalCtx>,
        state: Rc<SenderState>,
        enabled_individual: Guarded<bool>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        ctx.set_enabled_individual(*enabled_individual);
        if *enabled_individual {
            state.need_local_stream_update.set(true);
        } else {
            ctx.remove_track().await;
        }

        Ok(())
    }

    /// Watcher for the [`SenderState::enabled_general`] update.
    ///
    /// Calls [`Sender::set_enabled_general_state`] with a new value.
    #[watch(self.state().enabled_general.subscribe())]
    #[inline]
    async fn enabled_general_watcher(
        ctx: Rc<Sender>,
        _: Rc<GlobalCtx>,
        _: Rc<SenderState>,
        enabled_general: Guarded<bool>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        ctx.set_enabled_general(*enabled_general);

        Ok(())
    }

    /// Watcher for the [`SenderState::muted`] update.
    ///
    /// Calls [`Sender::set_muted`] with a new value.
    #[watch(self.state().muted.subscribe())]
    #[inline]
    async fn muted_watcher(
        ctx: Rc<Sender>,
        _: Rc<GlobalCtx>,
        _: Rc<SenderState>,
        muted: Guarded<bool>,
    ) -> Result<(), Traced<MediaConnectionsError>> {
        ctx.set_muted(*muted);

        Ok(())
    }
}
