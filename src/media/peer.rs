use std::{any::Any, sync::Arc};

use hashbrown::HashMap;

use crate::{
    api::client::Event,
    api::control::member::Id as MemberID,
    log::prelude::*,
    media::{
        errors::MediaError,
        track::{DirectionalTrack, Id as TrackID, Track, TrackDirection},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct New {}
#[derive(Debug, Clone, PartialEq)]
pub struct WaitLocalSDP {}
#[derive(Debug, Clone, PartialEq)]
pub struct WaitLocalHaveRemote {}
#[derive(Debug, Clone, PartialEq)]
pub struct WaitRemoteSDP {}
#[derive(Debug, Clone, PartialEq)]
pub struct Stable {}
#[derive(Debug, Clone, PartialEq)]
pub struct Finished {}
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {}

/// Implementation state machine for [`Peer`].
#[derive(Debug, Clone)]
pub enum PeerMachine {
    New(Peer<New>),
    WaitLocalSDP(Peer<WaitLocalSDP>),
    WaitLocalHaveRemote(Peer<WaitLocalHaveRemote>),
    WaitRemoteSDP(Peer<WaitRemoteSDP>),
    Stable(Peer<Stable>),
    Finished(Peer<Finished>),
    Failure(Peer<Failure>),
}

impl PeerMachine {
    pub fn get_member_id(&self) -> MemberID {
        match self {
            PeerMachine::New(peer) => peer.get_member_id(),
            PeerMachine::WaitLocalSDP(peer) => peer.get_member_id(),
            PeerMachine::WaitLocalHaveRemote(peer) => peer.get_member_id(),
            PeerMachine::WaitRemoteSDP(peer) => peer.get_member_id(),
            PeerMachine::Stable(peer) => peer.get_member_id(),
            PeerMachine::Finished(peer) => peer.get_member_id(),
            PeerMachine::Failure(peer) => peer.get_member_id(),
        }
    }
}

/// ID of [`Peer`].
pub type Id = u64;

#[derive(Debug, Clone)]
pub struct PeerContext {
    id: Id,
    member_id: MemberID,
    sdp_offer: Option<String>,
    sdp_answer: Option<String>,
    receivers: HashMap<TrackID, Arc<Track>>,
    senders: HashMap<TrackID, Arc<Track>>,
}

/// [`RTCPeerConnection`] representation.
#[derive(Debug, Clone)]
pub struct Peer<S> {
    context: PeerContext,
    state: S,
}

impl<T: Any> Peer<T> {
    pub fn get_member_id(&self) -> MemberID {
        self.context.member_id
    }
}

impl Peer<New> {
    /// Creates new [`Peer`] for [`Member`].
    pub fn new(id: Id, member_id: MemberID) -> Self {
        let context = PeerContext {
            id,
            member_id,
            sdp_offer: None,
            sdp_answer: None,
            receivers: HashMap::new(),
            senders: HashMap::new(),
        };
        Peer {
            context,
            state: New {},
        }
    }

    pub fn get_tracks(&self, opponent_id: Id) -> Vec<DirectionalTrack> {
        let tracks = self.context.senders.iter().fold(
            vec![],
            |mut tracks, (id, track)| {
                tracks.push(DirectionalTrack {
                    id: track.id,
                    media_type: track.media_type.clone(),
                    direction: TrackDirection::Send {
                        receivers: vec![opponent_id],
                    },
                });
                tracks
            },
        );
        self.context
            .receivers
            .iter()
            .fold(tracks, |mut tracks, (id, track)| {
                tracks.push(DirectionalTrack {
                    id: track.id,
                    media_type: track.media_type.clone(),
                    direction: TrackDirection::Recv {
                        sender: opponent_id,
                    },
                });
                tracks
            })
    }

    /// Sends PeerCreated event to Web Client and puts [`Peer`] into state
    /// of waiting for local offer.
    pub fn start(
        self,
        opponent_id: Id,
        success: impl FnOnce(MemberID, Event) -> (),
    ) -> Peer<WaitLocalSDP> {
        let tracks = self.get_tracks(opponent_id);
        let event = Event::PeerCreated {
            peer_id: self.context.id,
            sdp_offer: None,
            tracks,
        };
        success(self.context.member_id, event);
        Peer {
            context: self.context,
            state: WaitLocalSDP {},
        }
    }

    /// Sends PeerCreated event with local offer to Web Client and puts [`Peer`]
    /// into state of waiting for remote offer.
    pub fn set_remote_sdp(
        self,
        opponent_id: Id,
        sdp_offer: String,
        success: impl Fn(MemberID, Event) -> (),
    ) -> Peer<WaitLocalHaveRemote> {
        let tracks = self.get_tracks(opponent_id);
        let mut context = self.context;
        context.sdp_offer = Some(sdp_offer);
        let event = Event::PeerCreated {
            peer_id: context.id,
            sdp_offer: context.sdp_offer.clone(),
            tracks,
        };
        success(context.member_id, event);
        Peer {
            context,
            state: WaitLocalHaveRemote {},
        }
    }

    pub fn add_sender(&mut self, track: Arc<Track>) {
        self.context.senders.insert(track.id, track);
    }

    pub fn add_receiver(&mut self, track: Arc<Track>) {
        self.context.receivers.insert(track.id, track);
    }
}

#[test]
fn create_peer() {
    let peer = Peer::new(1, 1);
    let peer = peer.start(2, |_, _| {});

    assert_eq!(peer.state, WaitLocalSDP {});
}

impl Peer<WaitLocalSDP> {
    pub fn set_local_sdp(self, sdp_offer: String) -> Peer<WaitRemoteSDP> {
        let mut context = self.context;
        context.sdp_offer = Some(sdp_offer);
        Peer {
            context,
            state: WaitRemoteSDP {},
        }
    }
}

impl Peer<WaitRemoteSDP> {
    pub fn set_remote_sdp(
        self,
        sdp_answer: String,
        success: impl Fn(Id, MemberID, String) -> (),
    ) -> Peer<Stable> {
        let mut context = self.context;
        context.sdp_answer = Some(sdp_answer.clone());
        success(context.id, context.member_id, sdp_answer);
        Peer {
            context,
            state: Stable {},
        }
    }
}

impl Peer<WaitLocalHaveRemote> {
    pub fn set_local_sdp(self, sdp_answer: String) -> Peer<Stable> {
        let mut context = self.context;
        context.sdp_answer = Some(sdp_answer);
        Peer {
            context,
            state: Stable {},
        }
    }
}
