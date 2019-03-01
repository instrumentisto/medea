use crate::{api::control::member::Id as MemberID, log::prelude::*};

#[derive(Debug, Clone)]
pub struct New {}
#[derive(Debug, Clone)]
pub struct WaitLocalSDP {}
#[derive(Debug, Clone)]
pub struct WaitLocalHaveRemote {}
#[derive(Debug, Clone)]
pub struct WaitRemoteSDP {}
#[derive(Debug, Clone)]
pub struct Stable {}
#[derive(Debug, Clone)]
pub struct Finished {}
#[derive(Debug, Clone)]
pub struct Failure {}

/// ID of [`Peer`].
pub type Id = u64;

#[derive(Debug, Clone)]
pub struct PeerContext {
    id: Id,
    pub member_id: MemberID,
    pub opponent_peer_id: Option<Id>,
    offer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Peer<S> {
    pub context: PeerContext,
    state: S,
}

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

impl Peer<New> {
    pub fn new(id: Id, member_id: MemberID) -> Self {
        let context = PeerContext {
            id,
            member_id,
            opponent_peer_id: None,
            offer: None,
        };
        Peer {
            context,
            state: New {},
        }
    }

    pub fn start(self, opponent_peer_id: Id) -> Peer<WaitLocalSDP> {
        let mut context = self.context;
        context.opponent_peer_id = Some(opponent_peer_id);
        Peer {
            context,
            state: WaitLocalSDP {},
        }
    }

    pub fn set_remote_sdp(self, offer: &str) -> Peer<WaitLocalHaveRemote> {
        let mut context = self.context;
        context.offer = Some(offer.into());
        Peer {
            context,
            state: WaitLocalHaveRemote {},
        }
    }
}

impl Peer<WaitLocalSDP> {
    pub fn set_local_sdp(self, offer: &str) -> Peer<WaitRemoteSDP> {
        let mut context = self.context;
        context.offer = Some(offer.into());
        Peer {
            context,
            state: WaitRemoteSDP {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_peer() {
        let peer = Peer::new(1, 1);
        let peer = peer.start(2);

        assert_eq!(peer.context.opponent_peer_id, Some(2));
    }
}
