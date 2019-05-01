#![allow(clippy::use_self)]

use std::sync::Arc;

use hashbrown::HashMap;

use crate::api::protocol::Event;
use crate::{
    api::{
        control::MemberId,
        protocol::{
            AudioSettings, Direction, Directional, MediaType, VideoSettings,
        },
    },
    media::{Track, TrackId},
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
#[allow(clippy::module_name_repetitions)]
pub enum PeerStateMachine {
    New(Peer<New>),
    WaitLocalSDP(Peer<WaitLocalSDP>),
    WaitLocalHaveRemote(Peer<WaitLocalHaveRemote>),
    WaitRemoteSDP(Peer<WaitRemoteSDP>),
    Stable(Peer<Stable>),
}

// TODO: macro to remove boilerplate
impl PeerStateMachine {
    /// Returns ID of [`Member`] associated with this [`Peer`].
    pub fn member_id(&self) -> MemberId {
        match self {
            PeerStateMachine::New(peer) => peer.member_id(),
            PeerStateMachine::WaitLocalSDP(peer) => peer.member_id(),
            PeerStateMachine::WaitLocalHaveRemote(peer) => peer.member_id(),
            PeerStateMachine::WaitRemoteSDP(peer) => peer.member_id(),
            PeerStateMachine::Stable(peer) => peer.member_id(),
        }
    }

    /// Returns ID of [`Peer`].
    pub fn id(&self) -> Id {
        match self {
            PeerStateMachine::New(peer) => peer.id(),
            PeerStateMachine::WaitLocalSDP(peer) => peer.id(),
            PeerStateMachine::WaitLocalHaveRemote(peer) => peer.id(),
            PeerStateMachine::WaitRemoteSDP(peer) => peer.id(),
            PeerStateMachine::Stable(peer) => peer.id(),
        }
    }

    /// Returns ID of interconnected [`Peer`].
    pub fn partner_peer_id(&self) -> Id {
        match self {
            PeerStateMachine::New(peer) => peer.partner_peer_id(),
            PeerStateMachine::WaitLocalSDP(peer) => peer.partner_peer_id(),
            PeerStateMachine::WaitLocalHaveRemote(peer) => {
                peer.partner_peer_id()
            }
            PeerStateMachine::WaitRemoteSDP(peer) => peer.partner_peer_id(),
            PeerStateMachine::Stable(peer) => peer.partner_peer_id(),
        }
    }

    /// Returns ID of interconnected [`Member`].
    pub fn partner_member_id(&self) -> Id {
        match self {
            PeerStateMachine::New(peer) => peer.partner_peer_id(),
            PeerStateMachine::WaitLocalSDP(peer) => peer.partner_peer_id(),
            PeerStateMachine::WaitLocalHaveRemote(peer) => {
                peer.partner_peer_id()
            }
            PeerStateMachine::WaitRemoteSDP(peer) => peer.partner_peer_id(),
            PeerStateMachine::Stable(peer) => peer.partner_peer_id(),
        }
    }
}

/// ID of [`Peer`].
pub type Id = u64;

#[derive(Debug)]
pub struct Context {
    id: Id,
    member_id: MemberId,
    partner_peer: Id,
    partner_member: MemberId,
    sdp_offer: Option<String>,
    sdp_answer: Option<String>,
    receivers: HashMap<TrackId, Arc<Track>>,
    senders: HashMap<TrackId, Arc<Track>>,
}

/// [`RTCPeerConnection`] representation.
#[derive(Debug)]
pub struct Peer<S> {
    context: Context,
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

    /// Returns ID of interconnected [`Peer`].
    pub fn partner_peer_id(&self) -> Id {
        self.context.partner_peer
    }

    /// Returns ID of interconnected [`Member`].
    pub fn partner_member_id(&self) -> Id {
        self.context.partner_member
    }

    /// Returns [`Track`]'s of [`Peer`].
    pub fn tracks(&self) -> Vec<Directional> {
        let tracks = self.context.senders.iter().fold(
            vec![],
            |mut tracks, (_, track)| {
                tracks.push(Directional {
                    id: track.id,
                    media_type: track.media_type.clone(),
                    direction: Direction::Send {
                        receivers: vec![self.context.partner_peer],
                    },
                });
                tracks
            },
        );
        self.context
            .receivers
            .iter()
            .fold(tracks, |mut tracks, (_, track)| {
                tracks.push(Directional {
                    id: track.id,
                    media_type: track.media_type.clone(),
                    direction: Direction::Recv {
                        sender: self.context.partner_peer,
                    },
                });
                tracks
            })
    }

    pub fn is_sender(&self) -> bool {
        !self.context.senders.is_empty()
    }
}

impl Peer<New> {
    /// Creates new [`Peer`] for [`Member`].
    pub fn new(
        id: Id,
        member_id: MemberId,
        partner_peer: Id,
        partner_member: MemberId,
    ) -> Self {
        let context = Context {
            id,
            member_id,
            partner_peer,
            partner_member,
            sdp_offer: None,
            sdp_answer: None,
            receivers: HashMap::new(),
            senders: HashMap::new(),
        };
        Self {
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
    pub fn get_peer_created(&self) -> Event {
        Event::PeerCreated {
            peer_id: self.context.id,
            sdp_offer: self.context.sdp_offer.clone(),
            tracks: self.tracks(),
        }
    }

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
    pub fn set_remote_sdp(self, sdp_answer: &str) -> Peer<Stable> {
        let mut context = self.context;
        context.sdp_answer = Some(sdp_answer.to_string());
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

pub fn create_peers(
    caller: MemberId,
    responder: MemberId,
) -> HashMap<MemberId, PeerStateMachine> {
    let caller_peer_id = 1;
    let responder_peer_id = 2;
    let mut caller_peer =
        Peer::new(caller_peer_id, caller, responder_peer_id, responder_peer_id);
    let mut responder_peer =
        Peer::new(responder_peer_id, responder, caller_peer_id, caller_peer_id);

    let track_audio =
        Arc::new(Track::new(1, MediaType::Audio(AudioSettings {})));
    let track_video =
        Arc::new(Track::new(2, MediaType::Video(VideoSettings {})));
    caller_peer.add_sender(track_audio.clone());
    caller_peer.add_sender(track_video.clone());
    responder_peer.add_receiver(track_audio);
    responder_peer.add_receiver(track_video);

    hashmap!(
        caller_peer_id => PeerStateMachine::New(caller_peer),
        responder_peer_id => PeerStateMachine::New(responder_peer),
    )
}

#[test]
fn create_peer() {
    let peer = Peer::new(1, 1, 2, 2);
    let peer = peer.start();

    assert_eq!(peer.state, WaitLocalSDP {});
}
