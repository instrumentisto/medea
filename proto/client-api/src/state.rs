use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    IceCandidate, IceServer, MediaType, MemberId, NegotiationRole, PeerId,
    TrackId,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Room {
    pub peers: HashMap<PeerId, Peer>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Peer {
    pub id: PeerId,
    pub senders: HashMap<TrackId, Sender>,
    pub receivers: HashMap<TrackId, Receiver>,
    pub force_relay: bool,
    pub ice_servers: Vec<IceServer>,
    pub negotiation_role: Option<NegotiationRole>,
    pub sdp_offer: Option<String>,
    pub remote_sdp_offer: Option<String>,
    pub restart_ice: bool,
    pub ice_candidates: HashSet<IceCandidate>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Sender {
    pub id: TrackId,
    pub mid: Option<String>,
    pub media_type: MediaType,
    pub receivers: Vec<MemberId>,
    pub enabled_individual: bool,
    pub enabled_general: bool,
    pub muted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Receiver {
    pub id: TrackId,
    pub mid: Option<String>,
    pub media_type: MediaType,
    pub sender_id: MemberId,
    pub enabled_individual: bool,
    pub enabled_general: bool,
    pub muted: bool,
}
