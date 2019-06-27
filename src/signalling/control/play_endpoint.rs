use super::publish_endpoint::Id as PublishEndpointId;
use crate::api::control::MemberId;
use std::rc::Rc;
use crate::media::IceUser;
use crate::media::PeerId;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq)]
pub struct Id(String);

#[derive(Debug)]
pub struct WebRtcPlayEndpoint {
    id: Id,
    src: PublishEndpointId,
    owner: MemberId,
    ice_user: Option<Rc<IceUser>>,
    peer_id: Option<PeerId>
}
