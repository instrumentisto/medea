//! Remote [`RTCPeerConnection`][1] representation.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

#![allow(clippy::use_self)]
use failure::Fail;
use hashbrown::HashMap;
use medea_client_api_proto::{
    AudioSettings, Direction, MediaType, Track, VideoSettings,
};
use medea_macro::enum_delegate;

use std::{convert::TryFrom, fmt::Display, sync::Arc};

use crate::{
    api::control::{
        MemberId,
    },
    media::{MediaTrack, TrackId},
    signalling::peers::Counter,
    signalling::state::member::Participant,
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
#[allow(clippy::module_name_repetitions)]
#[enum_delegate(pub fn id(&self) -> Id)]
#[enum_delegate(pub fn member_id(&self) -> MemberId)]
#[enum_delegate(pub fn partner_peer_id(&self) -> Id)]
#[enum_delegate(pub fn partner_member_id(&self) -> MemberId)]
#[derive(Debug)]
pub enum PeerStateMachine {
    New(Peer<New>),
    WaitLocalSdp(Peer<WaitLocalSdp>),
    WaitLocalHaveRemote(Peer<WaitLocalHaveRemote>),
    WaitRemoteSdp(Peer<WaitRemoteSdp>),
    Stable(Peer<Stable>),
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
    receivers: HashMap<TrackId, Arc<MediaTrack>>,
    senders: HashMap<TrackId, Arc<MediaTrack>>,
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
        self.context.member_id.clone()
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
    pub fn partner_member_id(&self) -> MemberId {
        self.context.partner_member.clone()
    }

    /// Returns [`Track`]'s of [`Peer`].
    pub fn tracks(&self) -> Vec<Track> {
        let tracks = self.context.senders.iter().fold(
            vec![],
            |mut tracks, (_, track)| {
                tracks.push(Track {
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
                tracks.push(Track {
                    id: track.id,
                    media_type: track.media_type.clone(),
                    direction: Direction::Recv {
                        sender: self.context.partner_peer,
                    },
                });
                tracks
            })
    }

    /// Returns all senders [`MediaTrack`].
    pub fn get_senders(&self) -> Vec<Arc<MediaTrack>> {
        self.context
            .senders
            .iter()
            .map(|(_key, value)| value)
            .cloned()
            .collect()
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

    /// Add all [`WebRtcPublishEndpoint`]s to this [`Peer`].
    ///
    /// This use `track_id_counter` counter for generating new [`MediaTrack`]
    /// ID.
    pub fn add_publish_endpoints(
        &mut self,
        endpoints: HashMap<MemberId, Participant>,
        track_id_counter: &mut Counter,
    ) {
        for _ in endpoints {
            let track_audio = Arc::new(MediaTrack::new(
                track_id_counter.next_id(),
                MediaType::Audio(AudioSettings {}),
            ));
            let track_video = Arc::new(MediaTrack::new(
                track_id_counter.next_id(),
                MediaType::Video(VideoSettings {}),
            ));

            self.add_sender(track_audio);
            self.add_sender(track_video);
        }
    }

    /// Add all `partner_peer` related [`WebRtcPlayEndpoint`]s to this [`Peer`].
    pub fn add_play_endpoints(
        &mut self,
        endpoints: HashMap<MemberId, Participant>,
        partner_peer: &mut Peer<New>,
    ) {
        for (_, endpoint) in endpoints {
            if self.context.partner_member == endpoint.id() {
                partner_peer
                    .get_senders()
                    .into_iter()
                    .for_each(|s| self.add_receiver(s));
            }
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
    pub fn add_sender(&mut self, track: Arc<MediaTrack>) {
        self.context.senders.insert(track.id, track);
    }

    /// Add [`Track`] to [`Peer`] for receive.
    pub fn add_receiver(&mut self, track: Arc<MediaTrack>) {
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn create_peer() {
        let peer = Peer::new(
            1,
            MemberId(String::from("1")),
            2,
            MemberId(String::from("2")),
        );
        let peer = peer.start();

        assert_eq!(peer.state, WaitLocalSdp {});
    }
}
