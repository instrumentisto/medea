//! Remote [`RTCPeerConnection`][1] representation.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

#![allow(clippy::use_self)]

use std::{convert::TryFrom, fmt::Display, rc::Rc};

use failure::Fail;
use hashbrown::HashMap;
use medea_client_api_proto::{
    AudioSettings, Direction, MediaType, Track, VideoSettings,
};
use medea_macro::enum_delegate;

use crate::{
    api::control::MemberId,
    media::{MediaTrack, TrackId},
    signalling::{
        control::endpoint::{Id as EndpointId, WebRtcPublishEndpoint},
        peers::Counter,
    },
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
    receivers: HashMap<TrackId, Rc<MediaTrack>>,
    senders: HashMap<TrackId, Rc<MediaTrack>>,
}

/// [`RTCPeerConnection`] representation.
#[derive(Debug)]
pub struct Peer<S> {
    context: Context,
    state: S,
}

impl<T> Peer<T> {
    /// Returns ID of [`Participant`] associated with this [`Peer`].
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

    /// Returns ID of interconnected [`Participant`].
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
    pub fn get_senders(&self) -> Vec<Rc<MediaTrack>> {
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
    /// Creates new [`Peer`] for [`Participant`].
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

    /// Add all publish endpoints to this [`Peer`].
    ///
    /// This also create [`Peer`]s for [`WebRtcPlayEndpoint`]s that
    /// receive something from us.
    pub fn add_publish_endpoints(
        &mut self,
        partner_peer: &mut Peer<New>,
        tracks_count: &mut Counter,
        publish_endpoints: HashMap<EndpointId, Rc<WebRtcPublishEndpoint>>,
    ) {
        let partner_id = self.partner_member_id();
        let self_id = self.id();

        publish_endpoints
            .into_iter()
            .flat_map(|(_m, e)| {
                e.add_peer_id(self_id);
                e.receivers().into_iter().filter(|e| {
                    e.owner().id() == partner_id && !e.is_connected()
                })
            })
            .for_each(|e| {
                let track_audio = Rc::new(MediaTrack::new(
                    tracks_count.next_id(),
                    MediaType::Audio(AudioSettings {}),
                ));
                let track_video = Rc::new(MediaTrack::new(
                    tracks_count.next_id(),
                    MediaType::Video(VideoSettings {}),
                ));

                self.add_sender(track_video.clone());
                self.add_sender(track_audio.clone());

                partner_peer.add_receiver(track_video);
                partner_peer.add_receiver(track_audio);

                e.connect(partner_peer.id());
            });
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
    pub fn add_sender(&mut self, track: Rc<MediaTrack>) {
        self.context.senders.insert(track.id, track);
    }

    /// Add [`Track`] to [`Peer`] for receive.
    pub fn add_receiver(&mut self, track: Rc<MediaTrack>) {
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
