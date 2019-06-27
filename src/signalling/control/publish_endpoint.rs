use super::play_endpoint::{Id as PlayerEndpointId};
use crate::api::control::MemberId;
use std::rc::Rc;
use crate::media::IceUser;
use crate::media::PeerId;
use hashbrown::HashSet;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Id(String);

#[derive(Debug)]
pub struct WebRtcPublishEndpoint {
    id: Id,
    sinks: Vec<PlayerEndpointId>,
    owner: MemberId,
    ice_user: Option<Rc<IceUser>>,
    peer_ids: HashSet<PeerId>
}
