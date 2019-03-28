use actix::Message;
use serde::{Deserialize, Serialize};

use crate::{api::client::RoomError, media::peer::Id as PeerId};

/// WebSocket message from Web Client to Media Server.
#[derive(Debug, Deserialize, Message, Serialize)]
#[rtype(result = "Result<(), RoomError>")]
pub enum Command {
    /// Web Client sends SDP Offer.
    MakeSdpOffer { peer_id: PeerId, sdp_offer: String },
    /// Web Client sends SDP Answer.
    MakeSdpAnswer { peer_id: PeerId, sdp_answer: String },
    /// Web Client sends Ice Candidate.
    SetIceCandidate { peer_id: PeerId, candidate: String },
}
