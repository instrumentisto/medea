use crate::utils::{Component, ObservableSpawner};
use medea_client_api_proto::{
    MediaSourceKind, MediaType, MemberId, TrackId, TrackPatchEvent,
};
use medea_reactive::{Guarded, ObservableCell, ProgressableCell};

use crate::{api::RoomCtx, peer::Sender, MediaKind};
use futures::{future::LocalBoxFuture, stream::LocalBoxStream, StreamExt};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub type SenderComponent = Component<SenderState, Rc<Sender>, RoomCtx>;

pub struct SenderState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    receivers: Vec<MemberId>,
    enabled_individual: ProgressableCell<bool>,
    enabled_general: ProgressableCell<bool>,
    muted: ProgressableCell<bool>,
    need_local_stream_update: Cell<bool>,
}

impl SenderState {
    pub fn new(
        id: TrackId,
        mid: Option<String>,
        media_type: MediaType,
        receivers: Vec<MemberId>,
    ) -> Self {
        Self {
            id,
            mid,
            media_type,
            receivers,
            enabled_general: ProgressableCell::new(true),
            enabled_individual: ProgressableCell::new(true),
            muted: ProgressableCell::new(false),
            need_local_stream_update: Cell::new(false),
        }
    }

    pub fn id(&self) -> TrackId {
        self.id
    }

    pub fn mid(&self) -> &Option<String> {
        &self.mid
    }

    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    pub fn receivers(&self) -> &Vec<MemberId> {
        &self.receivers
    }

    pub fn enabled_individual(&self) -> bool {
        self.enabled_individual.get()
    }

    pub fn enabled_general(&self) -> bool {
        self.enabled_general.get()
    }

    pub fn update(&self, track_patch: TrackPatchEvent) {
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

    pub fn is_local_stream_update_needed(&self) -> bool {
        self.need_local_stream_update.get()
    }

    pub fn local_stream_updated(&self) {
        self.need_local_stream_update.set(false);
    }

    pub fn media_kind(&self) -> MediaKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaKind::Audio,
            MediaType::Video(_) => MediaKind::Video,
        }
    }

    pub fn media_source(&self) -> MediaSourceKind {
        match &self.media_type {
            MediaType::Audio(_) => MediaSourceKind::Device,
            MediaType::Video(video) => video.source_kind,
        }
    }
}

impl SenderComponent {
    pub fn spawn(&self) {
        self.spawn_task(
            self.state().enabled_individual.subscribe(),
            Self::handle_enabled_individual,
        );
        self.spawn_task(
            self.state().enabled_general.subscribe(),
            Self::handle_enabled_general,
        );
        self.spawn_task(self.state().muted.subscribe(), Self::handle_muted);
    }

    async fn handle_muted(
        ctx: Rc<Sender>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<SenderState>,
        muted: Guarded<bool>,
    ) {
        ctx.set_muted(*muted);
    }

    async fn handle_enabled_individual(
        ctx: Rc<Sender>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<SenderState>,
        enabled_individual: Guarded<bool>,
    ) {
        ctx.set_enabled_individual(*enabled_individual);
        if !*enabled_individual {
            ctx.remove_track().await;
        } else {
            state.need_local_stream_update.set(true);
        }
    }

    async fn handle_enabled_general(
        ctx: Rc<Sender>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<SenderState>,
        enabled_general: Guarded<bool>,
    ) {
        ctx.set_enabled_general(*enabled_general);
    }
}
