use serde::{Deserialize, Serialize};

// TODO: should be properly shared between medea and jason
#[derive(Deserialize)]
#[serde(untagged)]
pub enum ClientMsg {
    /// `pong` message that server answers with to WebSocket client in response
    /// to received `ping` message.
    #[serde(rename = "pong")]
    Pong(usize),
    #[serde(rename = "event")]
    Event(Event),
}

#[derive(Serialize)]
#[allow(dead_code)]
#[serde(untagged)]
pub enum ServerMsg {
    /// `ping` message that WebSocket client is expected to send to the server
    /// periodically.
    #[serde(rename = "ping")]
    Ping(usize),
    #[serde(rename = "command")]
    Command(Command),
}

/// WebSocket message from Web Client to Media Server.
#[derive(Serialize)]
#[allow(dead_code)]
pub enum Command {
    /// Web Client sends SDP Offer.
    MakeSdpOffer { peer_id: u64, sdp_offer: String },
    /// Web Client sends SDP Answer.
    MakeSdpAnswer { peer_id: u64, sdp_answer: String },
    /// Web Client sends Ice Candidate.
    SetIceCandidate { peer_id: u64, candidate: String },
}

/// WebSocket message from Medea to Jason.
#[derive(Deserialize)]
#[allow(dead_code)]
pub enum Event {
    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// creation.
    PeerCreated {
        peer_id: u64,
        sdp_offer: Option<String>,
        tracks: Vec<DirectionalTrack>,
    },
    /// Media Server notifies Web Client about necessity to apply specified SDP
    /// Answer to Web Client's RTCPeerConnection.
    SdpAnswerMade {
        peer_id: u64,
        sdp_answer: String,
    },

    IceCandidateDiscovered {
        peer_id: u64,
        candidate: String,
    },

    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// close.
    PeersRemoved {
        peer_ids: Vec<u64>,
    },
}

/// [`Track] with specified direction.
#[derive(Deserialize)]
pub struct DirectionalTrack {
    pub id: u64,
    pub direction: TrackDirection,
    pub media_type: TrackMediaType,
}

/// Direction of [`Track`].
#[derive(Deserialize)]
pub enum TrackDirection {
    Send { receivers: Vec<u64> },
    Recv { sender: u64 },
}

/// Type of [`Track`].
#[derive(Deserialize)]
pub enum TrackMediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

#[derive(Deserialize)]
pub struct AudioSettings {}

#[derive(Deserialize)]
pub struct VideoSettings {}
