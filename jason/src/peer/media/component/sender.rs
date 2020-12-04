use crate::utils::{Component, ObservableSpawner};
use medea_client_api_proto::{MediaType, MemberId, TrackId, TrackPatchEvent};
use medea_reactive::ObservableCell;

use crate::{api::RoomCtx, peer::Sender};
use std::rc::Rc;

pub type SenderComponent = Component<SenderState, Rc<Sender>, RoomCtx>;

pub struct SenderState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    receivers: Vec<MemberId>,
    enabled_individual: ObservableCell<bool>,
    enabled_general: ObservableCell<bool>,
    muted: ObservableCell<bool>,
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
            enabled_general: ObservableCell::new(true),
            enabled_individual: ObservableCell::new(true),
            muted: ObservableCell::new(false),
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
        muted: bool,
    ) {
        ctx.set_muted(muted);
    }

    async fn handle_enabled_individual(
        ctx: Rc<Sender>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<SenderState>,
        enabled_individual: bool,
    ) {
        ctx.set_enabled_individual(enabled_individual);
        if !enabled_individual {
            ctx.remove_track().await;
        }
    }

    async fn handle_enabled_general(
        ctx: Rc<Sender>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<SenderState>,
        enabled_general: bool,
    ) {
        ctx.set_enabled_general(enabled_general);
    }
}
