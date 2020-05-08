//! Snapshot of the `Peer` object.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{IceCandidate, IceServer, PeerId, TrackId, TrackPatch};

use super::{TrackSnapshot, TrackSnapshotAccessor};

/// Snapshot of the state for the `Peer`.
#[derive(Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct PeerSnapshot {
    /// ID of the `RTCPeerConnection`.
    pub id: PeerId,

    /// Current SDP offer of the `Peer`.
    pub sdp_offer: Option<String>,

    /// Current SDP answer of the `Peer`.
    pub sdp_answer: Option<String>,

    /// Snapshots of the all `MediaTrack`s of this `Peer`.
    pub tracks: HashMap<TrackId, TrackSnapshot>,

    /// Negotiated media IDs (mid) which the local and remote peers have agreed
    /// upon to uniquely identify the stream's pairing of sender and receiver
    /// for all `MediaTrack`s of this [`PeerSnapshot`].
    pub mids: HashMap<TrackId, String>,

    /// All [`IceServer`]s created for this `Peer`.
    pub ice_servers: HashSet<IceServer>,

    /// Indicates whether all media is forcibly relayed through a TURN server.
    pub is_force_relayed: bool,

    /// All [`IceCandidate`]s of this `Peer`.
    pub ice_candidates: HashSet<IceCandidate>,
}

/// Accessor to the `Peer` snapshot objects.
///
/// For this trait is implemented `CommandHandler` and
/// `EventHandler` which will be used on the Web Client side and on the Media
/// Server side. But real snapshot objects are different on the Web Client and
/// on the Media Server, so this abstraction is needed.
pub trait PeerSnapshotAccessor {
    type Track: TrackSnapshotAccessor;

    /// Returns new [`PeerSnapshotAccessor`] with provided data.
    fn new(
        id: PeerId,
        sdp_offer: Option<String>,
        ice_servers: HashSet<IceServer>,
        is_force_relayed: bool,
        tracks: HashMap<TrackId, Self::Track>,
        mids: HashMap<TrackId, String>,
    ) -> Self;

    /// Sets SDP answer for this `Peer`.
    fn set_sdp_answer(&mut self, sdp_answer: Option<String>);

    /// Sets SDP offer for this `Peer`.
    fn set_sdp_offer(&mut self, sdp_offer: Option<String>);

    /// Sets [`IceServer`]s list for this `Peer`.
    fn set_ice_servers(&mut self, ice_servers: HashSet<IceServer>);

    /// Sets `force_relay` setting of this `Peer`.
    fn set_is_force_relayed(&mut self, is_force_relayed: bool);

    /// Sets [`IceCandidate`]s list for this `Peer`.
    fn set_ice_candidates(&mut self, ice_candidates: HashSet<IceCandidate>);

    /// Adds new [`IceCandidate`] to this `Peer`.
    fn add_ice_candidate(&mut self, ice_candidate: IceCandidate);

    /// Updates `MediaTrack` with provided `track_id`.
    ///
    /// To `update_fn` will be provided mutable reference to the
    /// [`TrackSnapshotAccessor`] with which you can update `MediaTrack`.
    fn update_track<F>(&mut self, track_id: TrackId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Track>);

    /// Updates `MediaTrack`s of this `Peer` by provided [`TrackPatch`]s.
    fn update_tracks_by_patches(&mut self, patches: Vec<TrackPatch>) {
        for patch in patches {
            self.update_track(patch.id, |track| {
                if let Some(track) = track {
                    track.patch(patch);
                }
            });
        }
    }

    /// Updates this `Peer` state by provided [`PeerSnapshot`].
    fn update_snapshot(&mut self, snapshot: PeerSnapshot) {
        self.set_sdp_answer(snapshot.sdp_answer);
        self.set_sdp_offer(snapshot.sdp_offer);
        self.set_ice_candidates(snapshot.ice_candidates);
        self.set_ice_servers(snapshot.ice_servers);
        self.set_is_force_relayed(snapshot.is_force_relayed);

        for (track_id, track_snapshot) in snapshot.tracks {
            self.update_track(track_id, |track| {
                if let Some(track) = track {
                    track.update_snapshot(track_snapshot);
                }
            });
        }
    }

    fn extend_mids(&mut self, mids: HashMap<TrackId, String>);
}

impl PeerSnapshotAccessor for PeerSnapshot {
    type Track = TrackSnapshot;

    fn new(
        id: PeerId,
        sdp_offer: Option<String>,
        ice_servers: HashSet<IceServer>,
        is_force_relayed: bool,
        tracks: HashMap<TrackId, Self::Track>,
        mids: HashMap<TrackId, String>,
    ) -> Self {
        Self {
            id,
            sdp_offer,
            ice_servers,
            is_force_relayed,
            tracks,
            sdp_answer: None,
            ice_candidates: HashSet::new(),
            mids,
        }
    }

    fn set_sdp_answer(&mut self, sdp_answer: Option<String>) {
        self.sdp_answer = sdp_answer;
    }

    fn set_sdp_offer(&mut self, sdp_offer: Option<String>) {
        self.sdp_offer = sdp_offer
    }

    fn set_ice_servers(&mut self, ice_servers: HashSet<IceServer>) {
        self.ice_servers = ice_servers;
    }

    fn set_is_force_relayed(&mut self, is_force_relayed: bool) {
        self.is_force_relayed = is_force_relayed;
    }

    fn set_ice_candidates(&mut self, ice_candidates: HashSet<IceCandidate>) {
        self.ice_candidates = ice_candidates;
    }

    fn add_ice_candidate(&mut self, ice_candidate: IceCandidate) {
        self.ice_candidates.insert(ice_candidate);
    }

    fn update_track<F>(&mut self, track_id: TrackId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Track>),
    {
        (update_fn)(self.tracks.get_mut(&track_id));
    }

    fn extend_mids(&mut self, mids: HashMap<TrackId, String>) {
        self.mids.extend(mids.into_iter());
    }
}
