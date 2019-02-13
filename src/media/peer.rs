use crate::{api::control::member, log::prelude::*};

#[derive(Debug, PartialEq)]
pub enum PeerMachine {
    New(Peer),
    WaitLocalSDP(Peer),
    WaitLocalHaveRemote(Peer),
    WaitRemoteSDP(Peer),
    Stable(Peer),
    Finished(Peer),
    Failure,
}

pub enum Command {
    MakeSdpOffer,
    MakeSdpAnswer
}

/// ID of [`Peer`].
pub type Id = u64;

#[derive(Debug, PartialEq)]
struct Peer {
    id: Id,
    member_id: member::Id,
}

impl PeerMachine {
    pub fn new(id: Id, member_id: member::Id) -> Self {
        PeerMachine::New(Peer{id, member_id})
    }

    pub fn approve(self, c: Option<Command>) -> Self {
        match (self, c) {
            (PeerMachine::New(peer), None) => {
                PeerMachine::WaitLocalSDP(peer)
            },
            (PeerMachine::New(peer), Some(Command::MakeSdpOffer)) => {
                PeerMachine::WaitLocalHaveRemote(peer)
            },
            (PeerMachine::WaitLocalSDP(peer), Some(Command::MakeSdpOffer)) => {
                PeerMachine::WaitRemoteSDP(peer)
            },
            (PeerMachine::WaitLocalHaveRemote(peer), Some(Command::MakeSdpOffer)) => {
                PeerMachine::WaitRemoteSDP(peer)
            },
            _ => PeerMachine::Failure,
        }
    }

    pub fn pool(&self) {
        println!("{:?}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::*;

    #[test]
    fn create_peer() {
        let peer = PeerMachine::new(1, 1);
        let peer = peer.approve(None);
        peer.pool();

        assert_matches!(peer, PeerMachine::WaitLocalSDP(Peer{id: 1, member_id: 1}));
    }
}
