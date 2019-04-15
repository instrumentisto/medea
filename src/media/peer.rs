use std::sync::Arc;

use hashbrown::HashMap;

use crate::{
    api::control::member::Id as MemberId,
    media::track::{DirectionalTrack, Id as TrackId, Track, TrackDirection},
};

#[derive(Debug, PartialEq)]
pub struct New {}
#[derive(Debug, PartialEq)]
pub struct WaitLocalSDP {}
#[derive(Debug, PartialEq)]
pub struct WaitLocalHaveRemote {}
#[derive(Debug, PartialEq)]
pub struct WaitRemoteSDP {}
#[derive(Debug, PartialEq)]
pub struct Stable {}
#[derive(Debug, PartialEq)]
pub struct Finished {}
#[derive(Debug, PartialEq)]
pub struct Failure {}

/// Implementation state machine for [`Peer`].
#[derive(Debug)]
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
    /// Returns ID of [`Member`] associated with this [`Peer`].
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

    /// Returns ID of [`Peer`].
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

    /// Returns ID of [`Peer`].
    pub fn failed(self) -> Self {
        match self {
            PeerMachine::New(peer) => PeerMachine::Failure(peer.failed()),
            PeerMachine::WaitLocalSDP(peer) => {
                PeerMachine::Failure(peer.failed())
            }
            PeerMachine::WaitLocalHaveRemote(peer) => {
                PeerMachine::Failure(peer.failed())
            }
            PeerMachine::WaitRemoteSDP(peer) => {
                PeerMachine::Failure(peer.failed())
            }
            PeerMachine::Stable(peer) => PeerMachine::Failure(peer.failed()),
            PeerMachine::Finished(peer) => PeerMachine::Failure(peer.failed()),
            PeerMachine::Failure(peer) => PeerMachine::Failure(peer.failed()),
        }
    }

    /// Returns sender for this [`Peer`] if exists.
    pub fn sender(&self) -> Option<Id> {
        match self {
            PeerMachine::New(peer) => peer.sender(),
            PeerMachine::WaitLocalSDP(peer) => peer.sender(),
            PeerMachine::WaitLocalHaveRemote(peer) => peer.sender(),
            PeerMachine::WaitRemoteSDP(peer) => peer.sender(),
            PeerMachine::Stable(peer) => peer.sender(),
            PeerMachine::Finished(peer) => peer.sender(),
            PeerMachine::Failure(peer) => peer.sender(),
        }
    }

    /// Returns ID of interconnected [`Peer`].
    pub fn to_peer(&self) -> Id {
        match self {
            PeerMachine::New(peer) => peer.to_peer(),
            PeerMachine::WaitLocalSDP(peer) => peer.to_peer(),
            PeerMachine::WaitLocalHaveRemote(peer) => peer.to_peer(),
            PeerMachine::WaitRemoteSDP(peer) => peer.to_peer(),
            PeerMachine::Stable(peer) => peer.to_peer(),
            PeerMachine::Finished(peer) => peer.to_peer(),
            PeerMachine::Failure(peer) => peer.to_peer(),
        }
    }
}

/// ID of [`Peer`].
pub type Id = u64;

#[derive(Debug)]
pub struct PeerContext {
    id: Id,
    to_peer: Id,
    member_id: MemberId,
    sdp_offer: Option<String>,
    sdp_answer: Option<String>,
    receivers: HashMap<TrackId, Arc<Track>>,
    senders: HashMap<TrackId, Arc<Track>>,
}

/// [`RTCPeerConnection`] representation.
#[derive(Debug)]
pub struct Peer<S> {
    context: PeerContext,
    state: S,
}

impl<T> Peer<T> {
    /// Returns ID of [`Member`] associated with this [`Peer`].
    pub fn member_id(&self) -> MemberId {
        self.context.member_id
    }

    /// Returns ID of [`Peer`].
    pub fn id(&self) -> Id {
        self.context.id
    }

    pub fn failed(self) -> Peer<Failure> {
        Peer {
            context: self.context,
            state: Failure {},
        }
    }

    /// Returns ID of interconnected [`Peer`].
    pub fn to_peer(&self) -> Id {
        self.context.to_peer
    }

    /// Returns sender for this [`Peer`] if exists.
    pub fn sender(&self) -> Option<Id> {
        if self.context.receivers.is_empty() {
            None
        } else {
            Some(self.context.to_peer)
        }
    }

    /// Returns receiver for this [`Peer`] if exists.
    pub fn receiver(&self) -> Option<Id> {
        if self.context.senders.is_empty() {
            None
        } else {
            Some(self.context.to_peer)
        }
    }

    /// Returns [`Track`]'s of [`Peer`].
    pub fn tracks(&self) -> Vec<DirectionalTrack> {
        let tracks = self.context.senders.iter().fold(
            vec![],
            |mut tracks, (_, track)| {
                tracks.push(DirectionalTrack {
                    id: track.id,
                    media_type: track.media_type.clone(),
                    direction: TrackDirection::Send {
                        receivers: vec![self.context.to_peer],
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
                        sender: self.context.to_peer,
                    },
                });
                tracks
            })
    }
}

impl Peer<New> {
    /// Creates new [`Peer`] for [`Member`].
    pub fn new(id: Id, member_id: MemberId, to_peer: Id) -> Self {
        let context = PeerContext {
            id,
            member_id,
            to_peer,
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

    /// Transition new [`Peer`] into state of waiting for local description.
    pub fn start(self) -> Peer<WaitLocalSDP> {
        Peer {
            context: self.context,
            state: WaitLocalSDP {},
        }
    }

    /// Transition new [`Peer`] into state of waiting for remote description.
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

    /// Add [`Track`] to [`Peer`] for send.
    pub fn add_sender(&mut self, track: Arc<Track>) {
        self.context.senders.insert(track.id, track);
    }

    /// Add [`Track`] to [`Peer`] for receive.
    pub fn add_receiver(&mut self, track: Arc<Track>) {
        self.context.receivers.insert(track.id, track);
    }
}

impl Peer<WaitLocalSDP> {
    /// Set local description and transition [`Peer`]
    /// to [`WaitRemoteSDP`] state.
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
    /// Set remote description and transition [`Peer`] to [`Stable`] state.
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
    /// Set local description and transition [`Peer`] to [`Stable`] state.
    pub fn set_local_sdp(self, sdp_answer: String) -> Peer<Stable> {
        let mut context = self.context;
        context.sdp_answer = Some(sdp_answer);
        Peer {
            context,
            state: Stable {},
        }
    }
}

#[test]
fn create_peer() {
    let peer = Peer::new(1, 1, 2);
    let peer = peer.start();

    assert_eq!(peer.state, WaitLocalSDP {});
}
