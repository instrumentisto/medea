use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use futures::{Stream, StreamExt as _};
use medea_client_api_proto::{
    snapshots::peer::PeerSnapshotAccessor, IceCandidate, IceServer, PeerId,
    TrackId,
};
use medea_reactive::{collections::ObservableHashSet, Observable};

use super::ObservableTrackSnapshot;

/// Reactive snapshot of the state for the `Peer`.
#[derive(Debug)]
pub struct ObservablePeerSnapshot {
    /// ID of the `Peer`.
    pub id: PeerId,

    /// Current SDP offer of the `Peer`.
    pub sdp_offer: Observable<Option<String>>,

    /// Current SDP answer of the `Peer`.
    pub sdp_answer: Observable<Option<String>>,

    /// Snapshots of the all `MediaTrack`s of this `Peer`.
    pub tracks: HashMap<TrackId, Rc<RefCell<ObservableTrackSnapshot>>>,

    /// All [`IceServer`]s created for this `Peer`.
    pub ice_servers: ObservableHashSet<IceServer>,

    /// Indicates whether all media is forcibly relayed through a TURN server.
    pub is_force_relayed: Observable<bool>,

    /// All [`IceCandidate`]s of this `Peer`.
    pub ice_candidates: ObservableHashSet<IceCandidate>,
}

impl ObservablePeerSnapshot {
    /// Returns [`Stream`] to which will be sent SDP answer when it changes.
    pub fn on_sdp_answer_made(&self) -> impl Stream<Item = String> {
        self.sdp_answer.subscribe().filter_map(|new_sdp_answer| {
            Box::pin(async move { new_sdp_answer })
        })
    }

    /// Returns [`Stream`] to which will be sent new [`IceCandidate`] when it
    /// added to the snapshot.
    pub fn on_ice_candidate_discovered(
        &self,
    ) -> impl Stream<Item = IceCandidate> {
        self.ice_candidates.on_insert()
    }

    /// Returns all [`IceServer`]s created for this `Peer`.
    pub fn get_ice_servers(&self) -> Vec<IceServer> {
        self.ice_servers.iter().cloned().collect()
    }

    /// Returns indicator of whether all media is forcibly relayed through a
    /// TURN server.
    pub fn get_is_force_relayed(&self) -> bool {
        *self.is_force_relayed
    }

    /// Returns SDP offer of this `Peer`.
    pub fn get_sdp_offer(&self) -> &Option<String> {
        &self.sdp_offer
    }

    /// Returns references to the all [`ObservableTrackSnapshot`]s of this
    /// [`ObservablePeerSnapshot`].
    pub fn get_tracks(&self) -> Vec<Rc<RefCell<ObservableTrackSnapshot>>> {
        self.tracks.values().cloned().collect()
    }

    /// Returns [`PeerId`] of this [`ObservablePeerSnapshot`].
    pub fn get_id(&self) -> PeerId {
        self.id
    }
}

impl PeerSnapshotAccessor for ObservablePeerSnapshot {
    type Track = ObservableTrackSnapshot;

    fn new(
        id: PeerId,
        sdp_offer: Option<String>,
        ice_servers: HashSet<IceServer>,
        is_force_relayed: bool,
        tracks: HashMap<TrackId, Self::Track>,
    ) -> Self {
        ObservablePeerSnapshot {
            id,
            sdp_answer: Observable::new(None),
            sdp_offer: Observable::new(sdp_offer),
            ice_servers: ice_servers.into(),
            is_force_relayed: Observable::new(is_force_relayed),
            ice_candidates: ObservableHashSet::new(),
            tracks: tracks
                .into_iter()
                .map(|(id, track)| (id, Rc::new(RefCell::new(track))))
                .collect(),
        }
    }

    fn set_sdp_answer(&mut self, sdp_answer: Option<String>) {
        *self.sdp_answer.borrow_mut() = sdp_answer;
    }

    fn set_sdp_offer(&mut self, sdp_offer: Option<String>) {
        *self.sdp_offer.borrow_mut() = sdp_offer;
    }

    fn set_ice_servers(&mut self, ice_servers: HashSet<IceServer>) {
        self.ice_servers.update(ice_servers);
    }

    fn set_is_force_relayed(&mut self, is_force_relayed: bool) {
        *self.is_force_relayed.borrow_mut() = is_force_relayed;
    }

    fn set_ice_candidates(&mut self, ice_candidates: HashSet<IceCandidate>) {
        self.ice_candidates.update(ice_candidates);
    }

    fn add_ice_candidate(&mut self, ice_candidate: IceCandidate) {
        self.ice_candidates.insert(ice_candidate);
    }

    fn update_track<F>(&mut self, track_id: TrackId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Track>),
    {
        if let Some(track) = self.tracks.get(&track_id) {
            (update_fn)(Some(&mut track.borrow_mut()));
        } else {
            (update_fn)(None);
        }
    }
}
