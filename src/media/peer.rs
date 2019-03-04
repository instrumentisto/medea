use crate::api::control::member::Id as MemberID;

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

#[derive(Debug, Clone, PartialEq)]
pub struct PeerContext {
    id: Id,
    member_id: MemberID,
    sdp_offer: Option<String>,
    sdp_answer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Peer<S> {
    context: PeerContext,
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
            sdp_offer: None,
            sdp_answer: None,
        };
        Peer {
            context,
            state: New {},
        }
    }

    pub fn start(
        self,
        success: impl FnOnce(MemberID) -> (),
    ) -> Peer<WaitLocalSDP> {
        success(self.context.member_id);
        Peer {
            context: self.context,
            state: WaitLocalSDP {},
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_peer() {
        let peer = Peer::new(1, 1);
        let peer = peer.start(|_| {});

        assert_eq!(
            peer.context,
            PeerContext {
                id: 1,
                member_id: 1,
                sdp_offer: None,
                sdp_answer: None
            }
        );
    }
}
