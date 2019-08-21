//! Remote [`RTCPeerConnection`][1] representation.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

#![allow(clippy::use_self)]

use std::{
    collections::HashMap as StdHashMap, convert::TryFrom, fmt, sync::Arc,
};

use failure::Fail;
use hashbrown::HashMap;
use medea_client_api_proto::{
    AudioSettings, Direction, MediaType, Track, VideoSettings,
};
use medea_macro::enum_delegate;

use crate::{
    api::control::MemberId,
    media::{MediaTrack, TrackId},
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
pub enum PeerError {
    #[fail(
        display = "Cannot unwrap Peer from PeerStateMachine [id = {}]. \
                   Expected state {} was {}",
        _0, _1, _2
    )]
    WrongState(Id, &'static str, String),
    #[fail(
        display = "Peer is sending Track [{}] without providing its mid",
        _0
    )]
    MidsMismatch(TrackId),
}

impl PeerError {
    pub fn new_wrong_state(
        peer: &PeerStateMachine,
        expected: &'static str,
    ) -> Self {
        PeerError::WrongState(peer.id(), expected, format!("{}", peer))
    }
}

/// Implementation of ['Peer'] state machine.
#[allow(clippy::module_name_repetitions)]
#[enum_delegate(pub fn id(&self) -> Id)]
#[enum_delegate(pub fn member_id(&self) -> MemberId)]
#[enum_delegate(pub fn partner_peer_id(&self) -> Id)]
#[enum_delegate(pub fn partner_member_id(&self) -> Id)]
#[derive(Debug)]
pub enum PeerStateMachine {
    New(Peer<New>),
    WaitLocalSdp(Peer<WaitLocalSdp>),
    WaitLocalHaveRemote(Peer<WaitLocalHaveRemote>),
    WaitRemoteSdp(Peer<WaitRemoteSdp>),
    Stable(Peer<Stable>),
}

impl fmt::Display for PeerStateMachine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
            type Error = PeerError;

            fn try_from(
                peer: &'a PeerStateMachine,
            ) -> Result<Self, Self::Error> {
                match peer {
                    PeerStateMachine::$peer_type(peer) => Ok(peer),
                    _ => Err(PeerError::WrongState(
                        1,
                        stringify!($peer_type),
                        format!("{}", peer),
                    )),
                }
            }
        }

        impl TryFrom<PeerStateMachine> for Peer<$peer_type> {
            type Error = PeerError;

            fn try_from(peer: PeerStateMachine) -> Result<Self, Self::Error> {
                match peer {
                    PeerStateMachine::$peer_type(peer) => Ok(peer),
                    _ => Err(PeerError::WrongState(
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

/// [RTCPeerConnection] representation.
///
/// [RTCPeerConnection]: https://webrtcglossary.com/peerconnection/
#[derive(Debug)]
pub struct Peer<S> {
    context: Context,
    state: S,
}

impl<T> Peer<T> {
    /// Returns ID of [`Member`] associated with this [`Peer`].
    ///
    /// [`Member`]: crate::api::control::member::Member
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
    ///
    /// [`Member`]: crate::api::control::member::Member
    pub fn partner_member_id(&self) -> Id {
        self.context.partner_member
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
                        mid: track.mid(),
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
                        mid: track.mid(),
                    },
                });
                tracks
            })
    }

    /// Checks if this [`Peer`] has any send tracks.
    pub fn is_sender(&self) -> bool {
        !self.context.senders.is_empty()
    }
}

impl Peer<New> {
    /// Creates new [`Peer`] for [`Member`].
    ///
    /// [`Member`]: crate::api::control::member::Member
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
    pub fn add_sender(&mut self, track: Arc<MediaTrack>) {
        self.context.senders.insert(track.id, track);
    }

    /// Add [`Track`] to [`Peer`] for receive.
    pub fn add_receiver(&mut self, track: Arc<MediaTrack>) {
        self.context.receivers.insert(track.id, track);
    }
}

impl Peer<WaitLocalSdp> {
    /// Sets local description and transition [`Peer`]
    /// to [`WaitRemoteSdp`] state.
    pub fn set_local_sdp(self, sdp_offer: String) -> Peer<WaitRemoteSdp> {
        let mut context = self.context;
        context.sdp_offer = Some(sdp_offer);
        Peer {
            context,
            state: WaitRemoteSdp {},
        }
    }

    /// Sets tracks `mids`.
    ///
    /// Provided `mids` must have entries for all [`Peer`]s tracks.
    pub fn set_mids(
        &mut self,
        mut mids: StdHashMap<TrackId, String>,
    ) -> Result<(), PeerError> {
        for (id, track) in self
            .context
            .senders
            .iter_mut()
            .chain(self.context.receivers.iter_mut())
        {
            let mid = mids
                .remove(&id)
                .ok_or_else(|| PeerError::MidsMismatch(track.id))?;
            track.set_mid(mid)
        }
        Ok(())
    }
}

impl Peer<WaitRemoteSdp> {
    /// Sets remote description and transition [`Peer`] to [`Stable`] state.
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
    /// Sets local description and transition [`Peer`] to [`Stable`] state.
    pub fn set_local_sdp(self, sdp_answer: String) -> Peer<Stable> {
        let mut context = self.context;
        context.sdp_answer = Some(sdp_answer);
        Peer {
            context,
            state: Stable {},
        }
    }
}

impl Peer<Stable> {
    pub fn get_mids(&self) -> Result<StdHashMap<TrackId, String>, PeerError> {
        let mut mids = StdHashMap::with_capacity(self.context.senders.len());
        for (track_id, track) in self.context.senders.iter() {
            mids.insert(
                *track_id,
                track
                    .mid()
                    .ok_or_else(|| PeerError::MidsMismatch(track.id))?,
            );
        }
        Ok(mids)
    }
}

/// Creates 1<=>1 [`Room`].
///
/// [`Room`]: crate::signalling::Room
#[cfg(not(test))]
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
        Arc::new(MediaTrack::new(1, MediaType::Audio(AudioSettings {})));
    let track_video =
        Arc::new(MediaTrack::new(2, MediaType::Video(VideoSettings {})));
    caller_peer.add_sender(track_audio.clone());
    caller_peer.add_sender(track_video.clone());
    responder_peer.add_receiver(track_audio);
    responder_peer.add_receiver(track_video);

    let track_audio =
        Arc::new(MediaTrack::new(3, MediaType::Audio(AudioSettings {})));
    let track_video =
        Arc::new(MediaTrack::new(4, MediaType::Video(VideoSettings {})));
    responder_peer.add_sender(track_audio.clone());
    responder_peer.add_sender(track_video.clone());
    caller_peer.add_receiver(track_audio);
    caller_peer.add_receiver(track_video);

    hashmap!(
        caller_peer_id => PeerStateMachine::New(caller_peer),
        responder_peer_id => PeerStateMachine::New(responder_peer),
    )
}

/// Creates 1=>1 [`Room`].
#[cfg(test)]
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
        Arc::new(MediaTrack::new(1, MediaType::Audio(AudioSettings {})));
    let track_video =
        Arc::new(MediaTrack::new(2, MediaType::Video(VideoSettings {})));
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
}
