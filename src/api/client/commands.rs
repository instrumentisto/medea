use actix::Message;
use serde::{Deserialize, Serialize};

use crate::{
    api::client::RoomError, api::control::Id as MemberId,
    media::peer::Id as PeerId,
};

/// WebSocket message from Web Client to Media Server.
#[derive(Debug, Deserialize, Message, Serialize)]
#[rtype(result = "Result<(), RoomError>")]
pub enum Command {
    /// Web Client sends SDP Offer.
    MakeSdpOffer {
        member_id: MemberId,
        peer_id: PeerId,
        sdp_offer: String,
    },
    /// Web Client sends SDP Answer.
    MakeSdpAnswer {
        member_id: MemberId,
        peer_id: PeerId,
        sdp_answer: String,
    },

    SetIceCandidate {
        member_id: MemberId,
        peer_id: PeerId,
        candidate: String,
    },
}
