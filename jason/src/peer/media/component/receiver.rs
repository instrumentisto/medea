use medea_client_api_proto::{MediaType, MemberId, Track, TrackId};
use medea_reactive::{Observable, ObservableCell};

use crate::utils::Component;

use crate::{
    media::{RecvConstraints, TrackConstraints},
    peer::{MediaConnections, Receiver, TransceiverSide},
    utils::ObservableSpawner as _,
};
use std::rc::Rc;

pub type ReceiverComponent = Component<ReceiverState, Receiver>;

pub struct ReceiverState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    sender: MemberId,
    enabled_individual: ObservableCell<bool>,
    enabled_general: ObservableCell<bool>,
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
    }

    async fn handle_enabled_individual(
        ctx: Rc<Receiver>,
        enabled_individual: bool,
    ) {
        ctx.set_enabled_individual_state(enabled_individual);
    }

    async fn handle_enabled_general(ctx: Rc<Receiver>, enabled_general: bool) {
        ctx.set_enabled_general_state(enabled_general);
    }
}
