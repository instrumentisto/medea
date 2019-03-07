use std::sync::Arc;

use hashbrown::HashMap;

use crate::{
    api::control::member::Id as MemberID,
    media::{
        errors::MediaError,
        track::{Id as TrackID, Track},
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

    /// Sends PeerCreated event to Web Client and puts [`Peer`] into state
    /// of waiting for local offer.
    pub fn start(
        self,
        success: impl FnOnce(Id, MemberID) -> (),
    ) -> Peer<WaitLocalSDP> {
        success(self.context.id, self.context.member_id);
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
        success: impl Fn(Id, MemberID, String) -> (),
    ) -> Peer<WaitLocalHaveRemote> {
        let mut context = self.context;
        context.sdp_offer = Some(sdp_offer.clone());
        success(context.id, context.member_id, sdp_offer);
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
    let peer = peer.start(|_, _| {});

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
