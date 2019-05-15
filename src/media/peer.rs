//! Remote [`RTCPeerConnection`][1] representation.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

#![allow(clippy::use_self)]
use failure::Fail;
use hashbrown::HashMap;

use std::{convert::TryFrom, fmt::Display, sync::Arc};

use crate::{
    api::{
        control::MemberId,
        protocol::{
            AudioSettings, Direction, Directional, MediaType, VideoSettings,
        },
    },
    media::{Track, TrackId},
};

/// Newly initialized [`Peer`] ready to signalling.
#[derive(Debug, PartialEq)]
pub struct New {}

/// [`Peer`] doesnt have remote SDP and is waiting for local SDP.
#[derive(Debug, PartialEq)]
pub struct WaitLocalSdp {}

/// [`Peer`] has remote SDP and is waiting for local SDP.
#[derive(Debug, PartialEq)]
pub struct WaitLocalHaveRemote {}

/// [`Peer`] has local SDP and is waiting for remote SDP.
#[derive(Debug, PartialEq)]
pub struct WaitRemoteSdp {}

/// SDP exchange ended.
#[derive(Debug, PartialEq)]
pub struct Stable {}

/// Produced when unwrapping [`PeerStateMachine`] to [`Peer`] with wrong state.
#[derive(Fail, Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum PeerStateError {
    #[fail(
        display = "Cannot unwrap Peer from PeerStateMachine [id = {}]. \
                   Expected state {} was {}",
        _0, _1, _2
    )]
    WrongState(Id, &'static str, String),
}

impl PeerStateError {
    pub fn new_wrong_state(
        peer: &PeerStateMachine,
        expected: &'static str,
    ) -> Self {
        PeerStateError::WrongState(peer.id(), expected, format!("{}", peer))
    }
}

/// Implementation of ['Peer'] state machine.
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum PeerStateMachine {
    New(Peer<New>),
    WaitLocalSdp(Peer<WaitLocalSdp>),
    WaitLocalHaveRemote(Peer<WaitLocalHaveRemote>),
    WaitRemoteSdp(Peer<WaitRemoteSdp>),
    Stable(Peer<Stable>),
}

macro_rules! add_peer_state_machine_getter {
    ($function:tt, $result:ty) => {
        pub fn $function(&self) -> $result {
            match self {
                PeerStateMachine::New(peer) => peer.$function(),
                PeerStateMachine::WaitLocalSdp(peer) => peer.$function(),
                PeerStateMachine::WaitLocalHaveRemote(peer) => peer.$function(),
                PeerStateMachine::WaitRemoteSdp(peer) => peer.$function(),
                PeerStateMachine::Stable(peer) => peer.$function(),

            }
        }
    }
}

impl PeerStateMachine {
    /// Returns ID of [`Peer`].
    add_peer_state_machine_getter!(id, Id);

    /// Returns ID of [`Member`] associated with this [`Peer`].
    add_peer_state_machine_getter!(member_id, MemberId);

    /// Returns ID of interconnected [`Peer`].
    add_peer_state_machine_getter!(partner_peer_id, Id);

    /// Returns ID of interconnected [`Member`].
    add_peer_state_machine_getter!(partner_member_id, Id);
}

impl Display for PeerStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PeerStateMachine::WaitRemoteSdp(_) => write!(f, "WaitRemoteSdp"),
            PeerStateMachine::New(_) => write!(f, "New"),
            PeerStateMachine::WaitLocalSdp(_) => write!(f, "WaitLocalSdp"),
            PeerStateMachine::WaitLocalHaveRemote(_) => {
                write!(f, "WaitLocalHaveRemote")
            }
            PeerStateMachine::Stable(_) => write!(f, "Stable"),
        }
    }
}

macro_rules! impl_peer_converts {
    ($peer_type:tt) => {
        impl<'a> TryFrom<&'a PeerStateMachine> for &'a Peer<$peer_type> {
            type Error = PeerStateError;

            fn try_from(
                peer: &'a PeerStateMachine,
            ) -> Result<Self, Self::Error> {
                match peer {
                    PeerStateMachine::$peer_type(peer) => Ok(peer),
                    _ => Err(PeerStateError::WrongState(
                        1,
                        stringify!($peer_type),
                        format!("{}", peer),
                    )),
                }
            }
        }

        impl TryFrom<PeerStateMachine> for Peer<$peer_type> {
            type Error = PeerStateError;

            fn try_from(peer: PeerStateMachine) -> Result<Self, Self::Error> {
                match peer {
                    PeerStateMachine::$peer_type(peer) => Ok(peer),
                    _ => Err(PeerStateError::WrongState(
                        1,
                        stringify!($peer_type),
                        format!("{}", peer),
                    )),
                }
            }
        }

        impl From<Peer<$peer_type>> for PeerStateMachine {
            fn from(peer: Peer<$peer_type>) -> Self {
                PeerStateMachine::$peer_type(peer)
            }
        }
    };
}

impl_peer_converts!(New);
impl_peer_converts!(WaitLocalSdp);
impl_peer_converts!(WaitLocalHaveRemote);
impl_peer_converts!(WaitRemoteSdp);
impl_peer_converts!(Stable);

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
    pub fn start(self) -> Peer<WaitLocalSdp> {
        Peer {
            context: self.context,
            state: WaitLocalSdp {},
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

impl Peer<WaitLocalSdp> {
    /// Set local description and transition [`Peer`]
    /// to [`WaitRemoteSDP`] state.
    pub fn set_local_sdp(self, sdp_offer: String) -> Peer<WaitRemoteSdp> {
        let mut context = self.context;
        context.sdp_offer = Some(sdp_offer);
        Peer {
            context,
            state: WaitRemoteSdp {},
        }
    }
}

impl Peer<WaitRemoteSdp> {
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn create_peer() {
        let peer = Peer::new(1, 1, 2, 2);
        let peer = peer.start();

        assert_eq!(peer.state, WaitLocalSdp {});
    }

    #[test]
    fn should_get_values_from_getters() {
        let id = 1;
        let member_id = 1;
        let partner_peer = 2;
        let partner_member = 2;
        let peer = Peer::new(id, member_id, partner_peer, partner_member);

        assert_eq!(peer.id(), id);
        assert_eq!(peer.member_id(), member_id);
        assert_eq!(peer.partner_member_id(), partner_member);
        assert_eq!(peer.partner_peer_id(), partner_peer);
    }
}
