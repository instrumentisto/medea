use actix::Message;
use serde::{Deserialize, Serialize};

use crate::media::{peer::Id as PeerId, track::DirectionalTrack};

/// WebSocket message from Media Server to Web Client.
#[derive(Debug, Deserialize, Message, Serialize)]
pub enum Event {
    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// creation.
    PeerCreated {
        peer_id: PeerId,
        sdp_offer: Option<String>,
        tracks: Vec<DirectionalTrack>,
    },
    /// Media Server notifies Web Client about necessity to apply specified SDP
    /// Answer to Web Client's RTCPeerConnection.
    SdpAnswerMade {
        peer_id: PeerId,
        sdp_answer: String,
    },

    IceCandidateDiscovered {
        peer_id: PeerId,
        candidate: String,
    },
}
