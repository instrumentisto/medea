// use medea_client_api_proto::{TrackId, MediaType, MemberId};
// use medea_reactive::{ObservableCell};
// use crate::utils::Component;
// use crate::utils::ObservableSpawner;
//
// use crate::peer::Sender;
// use std::rc::Rc;
//
// pub type SenderComponent = Component<SenderState, Sender>;
//
// pub struct SenderState {
//     id: TrackId,
//     mid: Option<String>,
//     media_type: MediaType,
//     receivers: Vec<MemberId>,
//     enabled_individual: ObservableCell<bool>,
//     enabled_general: ObservableCell<bool>,
// }
//
// impl SenderComponent {
//     pub fn spawn(&self) {
//         self.spawn_task(
//             self.state().enabled_individual.subscribe(),
//             Self::handle_enabled_individual,
//         );
//         self.spawn_task(
//             self.state().enabled_general.subscribe(),
//             Self::handle_enabled_general,
//         );
//     }
//
//     fn handle_enabled_individual(ctx: Rc<Sender>, enabled_individual: bool) {
//
//     }
//
//     fn handle_enabled_general(ctx: Rc<Sender>, enabled_general: bool) {
//
//     }
// }