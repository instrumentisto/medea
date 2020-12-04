use medea_client_api_proto::{
    MediaType, MemberId, Track, TrackId, TrackPatchEvent,
};
use medea_reactive::{Observable, ObservableCell};

use crate::utils::Component;

use crate::{
    api::RoomCtx,
    media::{RecvConstraints, TrackConstraints},
    peer::{MediaConnections, Receiver, TransceiverSide},
    utils::ObservableSpawner as _,
};
use std::rc::Rc;

pub type ReceiverComponent = Component<ReceiverState, Rc<Receiver>, RoomCtx>;

pub struct ReceiverState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    sender: MemberId,
    enabled_individual: ObservableCell<bool>,
    enabled_general: ObservableCell<bool>,
    muted: ObservableCell<bool>,
}

impl ReceiverState {
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
            sender,
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

    pub fn sender(&self) -> &MemberId {
        &self.sender
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

impl ReceiverComponent {
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
        ctx: Rc<Receiver>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<ReceiverState>,
        muted: bool,
    ) {
        ctx.set_muted(muted);
    }

    async fn handle_enabled_individual(
        ctx: Rc<Receiver>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<ReceiverState>,
        enabled_individual: bool,
    ) {
        ctx.set_enabled_individual_state(enabled_individual);
    }

    async fn handle_enabled_general(
        ctx: Rc<Receiver>,
        global_ctx: Rc<RoomCtx>,
        state: Rc<ReceiverState>,
        enabled_general: bool,
    ) {
        ctx.set_enabled_general_state(enabled_general);
    }
}
