use medea_client_api_proto::{TrackId, MediaType, MemberId};
use medea_reactive::{ObservableCell, Observable};

use crate::utils::{Component};

use crate::peer::{Receiver, TransceiverSide, MediaConnections};
use std::rc::Rc;
use crate::media::{TrackConstraints, RecvConstraints};
use crate::utils::ObservableSpawner as _;

pub type ReceiverComponent = Component<ReceiverState, Receiver>;

pub struct ReceiverState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    sender: MemberId,
    enabled_individual: ObservableCell<bool>,
    enabled_general: ObservableCell<bool>,
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

    async fn handle_enabled_individual(ctx: Rc<Receiver>, enabled_individual: bool) {
        ctx.set_enabled_individual_state(enabled_individual);
    }

    async fn handle_enabled_general(ctx: Rc<Receiver>, enabled_general: bool) {
        ctx.set_enabled_general_state(enabled_general);
    }
}
