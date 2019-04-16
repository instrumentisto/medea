use actix::Message;
use serde::{Deserialize, Serialize};

use crate::media::{peer::Id as PeerId, track::Directional};

/// WebSocket message from Media Server to Web Client.
#[derive(Clone, Debug, Deserialize, Message, Serialize)]
pub enum Event {
    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// creation.
    PeerCreated {
        peer_id: PeerId,
        sdp_offer: Option<String>,
        tracks: Vec<Directional>,
    },
    /// Media Server notifies Web Client about necessity to apply specified SDP
    /// Answer to Web Client's RTCPeerConnection.
    SdpAnswerMade { peer_id: PeerId, sdp_answer: String },

    /// Media Server notifies Web Client about necessity to apply specified
    /// ICE Candidate.
    IceCandidateDiscovered { peer_id: PeerId, candidate: String },

    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// close.
    PeersRemoved { peer_ids: Vec<PeerId> },
}
