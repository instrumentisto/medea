use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    IceCandidate, IceServer, MediaType, MemberId, NegotiationRole, PeerId,
    TrackId,
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct State {
    // Y
    pub peers: HashMap<PeerId, PeerState>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PeerState {
    // Y
    pub id: PeerId,

    // See SenderState
    pub senders: HashMap<TrackId, SenderState>,

    // See ReceiverState
    pub receivers: HashMap<TrackId, ReceiverState>,

    // Maybe shouldn't be in State???
    pub ice_servers: Vec<IceServer>,

    // Nope and maybe should be removed from the State
    pub ice_candidates: Vec<IceCandidate>,

    // Y
    pub force_relay: bool,

    // Y
    pub negotiation_role: Option<NegotiationRole>,

    // Y, but we should somehow merge sdp_offer with a sdp_answer. Or just
    // merge it on server side.
    pub sdp_offer: Option<String>,

    // Y
    pub remote_sdp_offer: Option<String>,

    // Represented as TrackChange, so we should somehow bind it to this bool
    pub restart_ice: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SenderState {
    // Y
    pub id: TrackId,
    // Y
    pub mid: Option<String>,
    // Y
    pub media_type: MediaType,
    // Yep, vec![peer.partner_member_id]
    pub receivers: Vec<MemberId>,
    // Yep, track.enabled_send/enabled_recv
    pub enabled_individual: bool,
    // Yep, track.is_media_exchange_enabled
    pub enabled_general: bool,
    // Yep, track.is_muted_send/muted_recv
    pub muted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReceiverState {
    // Y
    pub id: TrackId,
    // Y
    pub mid: Option<String>,
    // Y
    pub media_type: MediaType,
    // Yep, peer.partner_member_id
    pub sender_id: MemberId,
    // Yep, track.enabled_send/enabled_recv
    pub enabled_individual: bool,
    // Yep, track.is_media_exchange_enabled
    pub enabled_general: bool,
    // Yep, track.is_muted_send/muted_recv
    pub muted: bool,
}
