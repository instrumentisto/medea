//! State of the Media Server which will be used for Client and Server
//! synchronization.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    IceCandidate, IceServer, MediaType, MemberId, NegotiationRole, PeerId,
    TrackId,
};

/// State of the `Room` element.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Room {
    /// All [`Peer`]s of this [`Room`].
    pub peers: HashMap<PeerId, Peer>,
}

/// State of the `Peer` element.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Peer {
    /// ID of the [`Peer`].
    pub id: PeerId,

    /// All [`Sender`]s of this [`Peer`].
    pub senders: HashMap<TrackId, Sender>,

    /// All [`Receiver`]s of this [`Peer`].
    pub receivers: HashMap<TrackId, Receiver>,

    /// Flag which indicates that this [`Peer`] should relay all media through
    /// a TURN server forcibly.
    pub force_relay: bool,

    /// List of [`IceServer`]s which this [`Peer`] should use.
    pub ice_servers: Vec<IceServer>,

    /// Current [`NegotiationRole`] of this [`Peer`].
    pub negotiation_role: Option<NegotiationRole>,

    /// Current SDP offer of this [`Peer`].
    pub sdp_offer: Option<String>,

    /// Current SDP offer of the partner [`Peer`].
    pub remote_sdp_offer: Option<String>,

    /// Flag which indicates that ICE restart should be performed.
    pub restart_ice: bool,

    /// All [`IceCandidate`]s of this [`Peer`].
    pub ice_candidates: HashSet<IceCandidate>,
}

/// State of the `MediaTrack`s with a `Send` direction.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Sender {
    /// ID of this [`Sender`].
    pub id: TrackId,

    /// Mid of this [`Sender`].
    pub mid: Option<String>,

    /// [`MediaType`] of this [`Sender`].
    pub media_type: MediaType,

    /// All `Member`s which are receives media from this [`Sender`].
    pub receivers: Vec<MemberId>,

    /// Flag which indicates that this [`Sender`] is enabled on `Send`
    /// direction side.
    pub enabled_individual: bool,

    /// Flag which indicates that this [`Sender`] is enabled on `Send` __and__
    /// `Recv` direction sides.
    pub enabled_general: bool,

    /// Flag which indicates that this [`Sender`] is muted.
    pub muted: bool,
}

/// State of the `MediaTrack`s with a `Recv` direction.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Receiver {
    /// ID of this [`Receiver`].
    pub id: TrackId,

    /// Mid of this [`Receiver`].
    pub mid: Option<String>,

    /// [`MediaType`] of this [`Receiver`].
    pub media_type: MediaType,

    /// `Member`s which sends media to this [`Receiver`].
    pub sender_id: MemberId,

    /// Flag which indicates that this [`Receiver`] is enabled on `Recv`
    /// direction side.
    pub enabled_individual: bool,

    /// Flag which indicates that this [`Receiver`] is enabled on `Send`
    /// __and__ `Recv` direction sides.
    pub enabled_general: bool,

    /// Flag which indicates that this [`Receiver`] is muted.
    pub muted: bool,
}
