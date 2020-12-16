use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    IceCandidate, IceServer, MediaType, MemberId, NegotiationRole, PeerId,
    TrackId,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct State {
    pub peers: HashMap<PeerId, PeerState>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PeerState {
    pub id: PeerId,
    pub senders: HashMap<TrackId, SenderState>,
    pub receivers: HashMap<TrackId, ReceiverState>,
    pub ice_servers: Vec<IceServer>,
    pub force_relay: bool,
    pub negotiation_role: Option<NegotiationRole>,
    pub sdp_offer: Option<String>,
    pub remote_sdp_offer: Option<String>,
    pub restart_ice: bool,
    pub ice_candidates: Vec<IceCandidate>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SenderState {
    pub id: TrackId,
    pub mid: Option<String>,
    pub media_type: MediaType,
    pub receivers: Vec<MemberId>,
    pub enabled_individual: bool,
    pub enabled_general: bool,
    pub muted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReceiverState {
    pub id: TrackId,
    pub mid: Option<String>,
    pub media_type: MediaType,
    pub sender_id: MemberId,
    pub enabled_individual: bool,
    pub enabled_general: bool,
    pub muted: bool,
}
