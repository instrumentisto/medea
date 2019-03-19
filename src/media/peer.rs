use std::{any::Any, sync::Arc};

use hashbrown::HashMap;

use crate::{
    api::client::Event,
    api::control::member::Id as MemberId,
    log::prelude::*,
    media::track::{DirectionalTrack, Id as TrackId, Track, TrackDirection},
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
    pub fn member_id(&self) -> MemberId {
        match self {
            PeerMachine::New(peer) => peer.member_id(),
            PeerMachine::WaitLocalSDP(peer) => peer.member_id(),
            PeerMachine::WaitLocalHaveRemote(peer) => peer.member_id(),
            PeerMachine::WaitRemoteSDP(peer) => peer.member_id(),
            PeerMachine::Stable(peer) => peer.member_id(),
            PeerMachine::Finished(peer) => peer.member_id(),
            PeerMachine::Failure(peer) => peer.member_id(),
        }
    }

    pub fn id(&self) -> Id {
        match self {
            PeerMachine::New(peer) => peer.id(),
            PeerMachine::WaitLocalSDP(peer) => peer.id(),
            PeerMachine::WaitLocalHaveRemote(peer) => peer.id(),
            PeerMachine::WaitRemoteSDP(peer) => peer.id(),
            PeerMachine::Stable(peer) => peer.id(),
            PeerMachine::Finished(peer) => peer.id(),
            PeerMachine::Failure(peer) => peer.id(),
        }
    }

    pub fn opponent_id(&self) -> Id {
        match self {
            PeerMachine::New(peer) => peer.opponent_id(),
            PeerMachine::WaitLocalSDP(peer) => peer.opponent_id(),
            PeerMachine::WaitLocalHaveRemote(peer) => peer.opponent_id(),
            PeerMachine::WaitRemoteSDP(peer) => peer.opponent_id(),
            PeerMachine::Stable(peer) => peer.opponent_id(),
            PeerMachine::Finished(peer) => peer.opponent_id(),
            PeerMachine::Failure(peer) => peer.opponent_id(),
        }
    }
}

/// ID of [`Peer`].
pub type Id = u64;

#[derive(Debug, Clone)]
pub struct PeerContext {
    id: Id,
    opponent_id: Id,
    member_id: MemberId,
    sdp_offer: Option<String>,
    sdp_answer: Option<String>,
    receivers: HashMap<TrackId, Arc<Track>>,
    senders: HashMap<TrackId, Arc<Track>>,
}

/// [`RTCPeerConnection`] representation.
#[derive(Debug, Clone)]
pub struct Peer<S> {
    context: PeerContext,
    state: S,
}

impl<T: Any> Peer<T> {
    pub fn member_id(&self) -> MemberId {
        self.context.member_id
    }

    pub fn id(&self) -> Id {
        self.context.id
    }

    pub fn opponent_id(&self) -> Id {
        self.context.opponent_id
    }

    pub fn tracks(&self) -> Vec<DirectionalTrack> {
        let tracks = self.context.senders.iter().fold(
            vec![],
            |mut tracks, (id, track)| {
                tracks.push(DirectionalTrack {
                    id: track.id,
                    media_type: track.media_type.clone(),
                    direction: TrackDirection::Send {
                        receivers: vec![self.context.opponent_id],
                    },
                });
                tracks
            },
        );
        self.context
            .receivers
            .iter()
            .fold(tracks, |mut tracks, (_, track)| {
                tracks.push(DirectionalTrack {
                    id: track.id,
                    media_type: track.media_type.clone(),
                    direction: TrackDirection::Recv {
                        sender: self.context.opponent_id,
                    },
                });
                tracks
            })
    }
}

impl Peer<New> {
    /// Creates new [`Peer`] for [`Member`].
    pub fn new(id: Id, member_id: MemberId, opponent_id: Id) -> Self {
        let context = PeerContext {
            id,
            opponent_id,
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

    /// Sends PeerCreated event to Web Client and puts [`Peer`] into state
    /// of waiting for local offer.
    pub fn start(self) -> Peer<WaitLocalSDP> {
        Peer {
            context: self.context,
            state: WaitLocalSDP {},
        }
    }

    /// Sends PeerCreated event with local offer to Web Client and puts [`Peer`]
    /// into state of waiting for remote offer.
    pub fn set_remote_sdp(
        self,
        sdp_offer: String,
    ) -> Peer<WaitLocalHaveRemote> {
        let mut context = self.context;
        context.sdp_offer = Some(sdp_offer);
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
    let peer = Peer::new(1, 1, 2);
    let peer = peer.start();

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
    pub fn set_remote_sdp(self, sdp_answer: String) -> Peer<Stable> {
        let mut context = self.context;
        context.sdp_answer = Some(sdp_answer.clone());
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
