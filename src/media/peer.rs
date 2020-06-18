//! Remote [`RTCPeerConnection`][1] representation.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

#![allow(clippy::use_self)]

use std::{collections::HashMap, convert::TryFrom, fmt, rc::Rc};

use derive_more::Display;
use failure::Fail;
use medea_client_api_proto::{
    AudioSettings, Direction, IceServer, MediaType, PeerId as Id, Track,
    TrackId, TrackUpdate, VideoSettings,
};
use medea_macro::enum_delegate;

use crate::{
    api::control::MemberId,
    media::{IceUser, MediaTrack},
    signalling::{
        elements::endpoints::{Endpoint, WeakEndpoint},
        peers::Counter,
    },
};

/// [`Peer`] doesnt have remote [SDP] and is waiting for local [SDP].
///
/// [SDP]: https://tools.ietf.org/html/rfc4317
#[derive(Debug, PartialEq)]
pub struct WaitLocalSdp;

/// [`Peer`] has remote [SDP] and is waiting for local [SDP].
///
/// [SDP]: https://tools.ietf.org/html/rfc4317
#[derive(Debug, PartialEq)]
pub struct WaitLocalHaveRemote;

/// [`Peer`] has local [SDP] and is waiting for remote [SDP].
///
/// [SDP]: https://tools.ietf.org/html/rfc4317
#[derive(Debug, PartialEq)]
pub struct WaitRemoteSdp;

/// There is no negotiation happening atm. It may have ended or haven't started
/// yet.
#[derive(Debug, PartialEq)]
pub struct Stable;

/// Produced when unwrapping [`PeerStateMachine`] to [`Peer`] with wrong state.
#[derive(Debug, Display, Fail)]
pub enum PeerError {
    #[display(
        fmt = "Cannot unwrap Peer from PeerStateMachine [id = {}]. Expected \
               state {} was {}",
        _0,
        _1,
        _2
    )]
    WrongState(Id, &'static str, String),
    #[display(
        fmt = "Peer is sending Track [{}] without providing its mid",
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
#[enum_delegate(pub fn id(&self) -> Id)]
#[enum_delegate(pub fn member_id(&self) -> MemberId)]
#[enum_delegate(pub fn partner_peer_id(&self) -> Id)]
#[enum_delegate(pub fn partner_member_id(&self) -> MemberId)]
#[enum_delegate(pub fn is_force_relayed(&self) -> bool)]
#[enum_delegate(pub fn ice_servers_list(&self) -> Option<Vec<IceServer>>)]
#[enum_delegate(pub fn set_ice_user(&mut self, ice_user: IceUser))]
#[enum_delegate(pub fn senders(&self) -> &HashMap<TrackId, Rc<MediaTrack>>)]
#[enum_delegate(
    pub fn receivers(&self) -> &HashMap<TrackId, Rc<MediaTrack>>
)]
#[derive(Debug)]
pub enum PeerStateMachine {
    WaitLocalSdp(Peer<WaitLocalSdp>),
    WaitLocalHaveRemote(Peer<WaitLocalHaveRemote>),
    WaitRemoteSdp(Peer<WaitRemoteSdp>),
    Stable(Peer<Stable>),
}

impl fmt::Display for PeerStateMachine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PeerStateMachine::WaitRemoteSdp(_) => write!(f, "WaitRemoteSdp"),
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
                        peer.id(),
                        stringify!($peer_type),
                        format!("{}", peer),
                    )),
                }
            }
        }

        impl TryFrom<PeerStateMachine> for Peer<$peer_type> {
            type Error = (PeerError, PeerStateMachine);

            fn try_from(peer: PeerStateMachine) -> Result<Self, Self::Error> {
                match peer {
                    PeerStateMachine::$peer_type(peer) => Ok(peer),
                    _ => Err((
                        PeerError::WrongState(
                            peer.id(),
                            stringify!($peer_type),
                            format!("{}", peer),
                        ),
                        peer,
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

impl_peer_converts!(WaitLocalSdp);
impl_peer_converts!(WaitLocalHaveRemote);
impl_peer_converts!(WaitRemoteSdp);
impl_peer_converts!(Stable);

#[derive(Debug)]
pub struct Context {
    /// [`PeerId`] of this [`Peer`].
    id: Id,

    /// [`MemberId`] of a [`Member`] which owns this [`Peer`].
    member_id: MemberId,

    /// [`PeerId`] of a partner [`Peer`].
    partner_peer: Id,

    /// [`MemberId`] of a partner [`Peer`]'s owner.
    partner_member: MemberId,

    /// [`IceUser`] created for this [`Peer`].
    ice_user: Option<IceUser>,

    /// [SDP] offer of this [`Peer`].
    ///
    /// [SDP]: https://tools.ietf.org/html/rfc4317
    sdp_offer: Option<String>,

    /// [SDP] answer of this [`Peer`].
    ///
    /// [SDP]: https://tools.ietf.org/html/rfc4317
    sdp_answer: Option<String>,

    /// All [`MediaTrack`]s with a `Recv` direction`.
    receivers: HashMap<TrackId, Rc<MediaTrack>>,

    /// All [`MediaTrack`]s with a `Send` direction.
    senders: HashMap<TrackId, Rc<MediaTrack>>,

    /// If `true` then this [`Peer`] must be forcibly connected through TURN.
    is_force_relayed: bool,

    /// Weak references to the [`Endpoint`]s related to this [`Peer`].
    endpoints: Vec<WeakEndpoint>,

    /// Status of renegotiation process.
    ///
    /// If `true` then renegotiation is going.
    ///
    /// If `false` then not renegotiation is in progress.
    is_renegotiate: bool,

    /// Tracks changes, that remote Peer is not aware of.
    pending_track_updates: Vec<TrackChange>,
}

/// Tracks changes, that remote Peer is not aware of.
#[derive(Debug)]
enum TrackChange {
    /// [`MediaTrack`]s with [`Direction::Send`] of this [`Peer`] that remote
    /// Peer is not aware of.
    AddSendTrack(Rc<MediaTrack>),
    /// [`MediaTrack`]s with [`Direction::Recv`] of this [`Peer`] that remote
    /// Peer is not aware of.
    AddRecvTrack(Rc<MediaTrack>),
}

impl TrackChange {
    fn try_as_track(&self, partner_peer_id: Id) -> Option<Track> {
        let (direction, track) = match self {
            TrackChange::AddSendTrack(track) => (
                Direction::Send {
                    receivers: vec![partner_peer_id],
                    mid: track.mid(),
                },
                track,
            ),
            TrackChange::AddRecvTrack(track) => (
                Direction::Recv {
                    sender: partner_peer_id,
                    mid: track.mid(),
                },
                track,
            ),
        };

        Some(Track {
            id: track.id,
            is_muted: false,
            media_type: track.media_type.clone(),
            direction,
        })
    }
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
    /// [`Member`]: crate::signalling::elements::member::Member
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
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    pub fn partner_member_id(&self) -> MemberId {
        self.context.partner_member.clone()
    }

    /// Returns [`TrackUpdate`]s of this [`Peer`] which should be sent to the
    /// client in the [`Event::TracksApplied`].
    pub fn get_updates(&self) -> Vec<TrackUpdate> {
        self.context
            .pending_track_updates
            .iter()
            .map(|change| change.try_as_track(self.partner_peer_id()).unwrap())
            .map(TrackUpdate::Added)
            .collect()
    }

    /// Returns [`Track`]s that remote Peer is not aware of.
    pub fn new_tracks(&self) -> Vec<Track> {
        self.context
            .pending_track_updates
            .iter()
            .filter_map(|update| update.try_as_track(self.partner_peer_id()))
            .collect()
    }

    /// Checks if this [`Peer`] has any send tracks.
    pub fn is_sender(&self) -> bool {
        !self.context.senders.is_empty()
    }

    /// Indicates whether all media is forcibly relayed through a TURN server.
    pub fn is_force_relayed(&self) -> bool {
        self.context.is_force_relayed
    }

    /// Returns vector of [`IceServer`]s built from this [`Peer`]s [`IceUser`].
    pub fn ice_servers_list(&self) -> Option<Vec<IceServer>> {
        self.context.ice_user.as_ref().map(IceUser::servers_list)
    }

    /// Sets [`IceUser`], which is used to generate [`IceServer`]s
    pub fn set_ice_user(&mut self, ice_user: IceUser) {
        self.context.ice_user.replace(ice_user);
    }

    /// Returns [`WeakEndpoint`]s for which this [`Peer`] was created.
    pub fn endpoints(&self) -> Vec<WeakEndpoint> {
        self.context.endpoints.clone()
    }

    /// Adds [`Endpoint`] for which this [`Peer`] was created.
    pub fn add_endpoint(&mut self, endpoint: &Endpoint) {
        match endpoint {
            Endpoint::WebRtcPlayEndpoint(play) => {
                play.set_peer_id(self.id());
            }
            Endpoint::WebRtcPublishEndpoint(publish) => {
                publish.add_peer_id(self.id());
            }
        }
        self.context.endpoints.push(endpoint.downgrade());
    }

    /// Returns all receiving [`MediaTrack`]s of this [`Peer`].
    pub fn receivers(&self) -> &HashMap<TrackId, Rc<MediaTrack>> {
        &self.context.receivers
    }

    /// Returns all sending [`MediaTrack`]s of this [`Peer`].
    pub fn senders(&self) -> &HashMap<TrackId, Rc<MediaTrack>> {
        &self.context.senders
    }

    /// Returns `true` if renegotiation is going.
    ///
    /// Returns `false` if no renegotiation is started.
    pub fn is_renegotiate(&self) -> bool {
        self.context.is_renegotiate
    }

    /// Resets [`Context::is_renegotiate`] to `false`.
    ///
    /// Resets `pending_changes` buffer.
    ///
    /// Should be called when renegotiation was finished.
    fn renegotiation_finished(&mut self) {
        // TODO: peer_created_on_remote / known_to_remote
        //       change on stable => any other transition
        self.context.is_renegotiate = false;
        self.context.pending_track_updates.clear();
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

    /// Sets tracks [mid]s.
    ///
    /// Provided [mid]s must have entries for all [`Peer`]s tracks.
    ///
    /// # Errors
    ///
    /// Errors with [`PeerError::MidsMismatch`] if [`Peer`] is sending
    /// [`MediaTrack`] without providing its [mid].
    ///
    /// [mid]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/RTCRtpTransceiver/mid
    pub fn set_mids(
        &mut self,
        mut mids: HashMap<TrackId, String>,
    ) -> Result<(), PeerError> {
        let tracks = self
            .context
            .senders
            .iter_mut()
            .chain(self.context.receivers.iter_mut());

        for (id, track) in tracks {
            let mid = mids
                .remove(&id)
                .ok_or_else(|| PeerError::MidsMismatch(track.id))?;
            track.set_mid(mid)
        }

        Ok(())
    }
}

impl Peer<WaitRemoteSdp> {
    /// Sets remote description and transitions [`Peer`] to [`Stable`] state.
    pub fn set_remote_sdp(mut self, sdp_answer: &str) -> Peer<Stable> {
        self.renegotiation_finished();
        self.context.sdp_answer = Some(sdp_answer.to_string());

        Peer {
            context: self.context,
            state: Stable {},
        }
    }
}

impl Peer<WaitLocalHaveRemote> {
    /// Sets local description and transitions [`Peer`] to [`Stable`] state.
    pub fn set_local_sdp(mut self, sdp_answer: String) -> Peer<Stable> {
        self.renegotiation_finished();
        self.context.sdp_answer = Some(sdp_answer);

        Peer {
            context: self.context,
            state: Stable {},
        }
    }
}

impl Peer<Stable> {
    /// Creates new [`Peer`] for [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    pub fn new(
        id: Id,
        member_id: MemberId,
        partner_peer: Id,
        partner_member: MemberId,
        is_force_relayed: bool,
    ) -> Self {
        let context = Context {
            id,
            member_id,
            partner_peer,
            partner_member,
            ice_user: None,
            sdp_offer: None,
            sdp_answer: None,
            receivers: HashMap::new(),
            senders: HashMap::new(),
            is_force_relayed,
            endpoints: Vec::new(),
            is_renegotiate: false,
            pending_track_updates: Vec::new(),
        };

        Self {
            context,
            state: Stable {},
        }
    }

    /// Adds `send` tracks to `self` and add `recv` for this `send`
    /// to `partner_peer`.
    pub fn add_publisher(
        &mut self,
        partner_peer: &mut Peer<Stable>,
        tracks_count: &mut Counter<TrackId>,
    ) {
        let track_audio = Rc::new(MediaTrack::new(
            tracks_count.next_id(),
            MediaType::Audio(AudioSettings {}),
        ));
        let track_video = Rc::new(MediaTrack::new(
            tracks_count.next_id(),
            MediaType::Video(VideoSettings {}),
        ));

        self.add_sender(Rc::clone(&track_video));
        self.add_sender(Rc::clone(&track_audio));

        partner_peer.add_receiver(track_video);
        partner_peer.add_receiver(track_audio);
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

    /// Returns [mid]s of this [`Peer`].
    ///
    /// # Errors
    ///
    /// Errors with [`PeerError::MidsMismatch`] if [`Peer`] is sending
    /// [`MediaTrack`] without providing its [mid].
    ///
    /// [mid]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/RTCRtpTransceiver/mid
    pub fn get_mids(&self) -> Result<HashMap<TrackId, String>, PeerError> {
        let mut mids = HashMap::with_capacity(self.context.senders.len());
        for (track_id, track) in &self.context.senders {
            mids.insert(
                *track_id,
                track
                    .mid()
                    .ok_or_else(|| PeerError::MidsMismatch(track.id))?,
            );
        }
        Ok(mids)
    }

    /// Changes [`Peer`] state to [`WaitLocalSdp`] and discards previously saved
    /// [SDP] Offer and Answer.
    ///
    /// Sets [`Context::is_renegotiate`] to `true`.
    ///
    /// Resets [`Context::sdp_offer`] and [`Context::sdp_answer`].
    ///
    /// [SDP]: https://tools.ietf.org/html/rfc4317
    pub fn start_renegotiation(self) -> Peer<WaitLocalSdp> {
        let mut context = self.context;
        context.is_renegotiate = true;
        context.sdp_answer = None;
        context.sdp_offer = None;

        Peer {
            context,
            state: WaitLocalSdp {},
        }
    }

    /// Adds [`Track`] to [`Peer`] send tracks list.
    ///
    /// This [`Track`] is considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks`.
    fn add_sender(&mut self, track: Rc<MediaTrack>) {
        self.context
            .pending_track_updates
            .push(TrackChange::AddSendTrack(Rc::clone(&track)));
        self.context.senders.insert(track.id, track);
    }

    /// Adds [`Track`] to [`Peer`] receive tracks list.
    ///
    /// This [`Track`] is considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks`.
    fn add_receiver(&mut self, track: Rc<MediaTrack>) {
        self.context
            .pending_track_updates
            .push(TrackChange::AddRecvTrack(Rc::clone(&track)));
        self.context.receivers.insert(track.id, track);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Returns [`PeerStateMachine`] with provided count of the `MediaTrack`s
    /// media types.
    pub fn test_peer_from_peer_tracks(
        send_audio: u32,
        send_video: u32,
        recv_audio: u32,
        recv_video: u32,
    ) -> PeerStateMachine {
        let mut peer = Peer::new(
            Id(1),
            MemberId::from("test-member"),
            Id(2),
            MemberId::from("partner-member"),
            false,
        );

        let mut track_id_counter = Counter::default();

        for _ in 0..send_audio {
            let track_id = track_id_counter.next_id();
            let track =
                MediaTrack::new(track_id, MediaType::Audio(AudioSettings {}));
            peer.context.senders.insert(track_id, Rc::new(track));
        }

        for _ in 0..send_video {
            let track_id = track_id_counter.next_id();
            let track =
                MediaTrack::new(track_id, MediaType::Video(VideoSettings {}));
            peer.context.senders.insert(track_id, Rc::new(track));
        }

        for _ in 0..recv_audio {
            let track_id = track_id_counter.next_id();
            let track =
                MediaTrack::new(track_id, MediaType::Audio(AudioSettings {}));
            peer.context.receivers.insert(track_id, Rc::new(track));
        }

        for _ in 0..recv_video {
            let track_id = track_id_counter.next_id();
            let track =
                MediaTrack::new(track_id, MediaType::Video(VideoSettings {}));
            peer.context.receivers.insert(track_id, Rc::new(track));
        }

        peer.into()
    }
}
