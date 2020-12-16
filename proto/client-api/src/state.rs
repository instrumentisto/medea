use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::{PeerId, IceCandidate, NegotiationRole, IceServer, TrackId, MediaType, MemberId};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct State {
    peers: HashMap<PeerId, PeerState>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PeerState {
    id: PeerId,
    senders: HashMap<TrackId, SenderState>,
    receivers: HashMap<TrackId, ReceiverState>,
    ice_servers: Vec<IceServer>,
    force_relay: bool,
    negotiation_role: Option<NegotiationRole>,
    sdp_offer: Option<String>,
    remote_sdp_offer: Option<String>,
    restart_ice: bool,
    ice_candidates: Vec<IceCandidate>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SenderState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    receivers: Vec<MemberId>,
    enabled_individual: bool,
    enabled_general: bool,
    muted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReceiverState {
    id: TrackId,
    mid: Option<String>,
    media_type: MediaType,
    sender_id: MemberId,
    enabled_individual: bool,
    enabled_general: bool,
    muted: bool,
}
