use crate::utils::{Component, ObservableSpawner};
use medea_client_api_proto::{MediaType, MemberId, TrackId};
use medea_reactive::ObservableCell;

use crate::peer::Sender;
use std::rc::Rc;

pub type SenderComponent = Component<SenderState, Rc<Sender>>;

pub struct SenderState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    receivers: Vec<MemberId>,
    enabled_individual: ObservableCell<bool>,
    enabled_general: ObservableCell<bool>,
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
            enabled_general: ObservableCell::new(false),
            enabled_individual: ObservableCell::new(false),
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
    }

    async fn handle_enabled_individual(
        ctx: Rc<Sender>,
        enabled_individual: bool,
    ) {
        ctx.set_enabled_individual(enabled_individual);
    }

    async fn handle_enabled_general(ctx: Rc<Sender>, enabled_general: bool) {
        ctx.set_enabled_general(enabled_general);
    }
}
