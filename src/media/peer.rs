//! Remote [`RTCPeerConnection`][1] representation.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

#![allow(clippy::use_self)]

use std::{
    collections::{HashMap, VecDeque},
    convert::TryFrom,
    fmt,
    rc::Rc,
};

use derive_more::Display;
use failure::Fail;
use medea_client_api_proto::{
    AudioSettings, Direction, IceServer, MediaType, PeerId as Id, PeerId,
    Track, TrackId, TrackUpdate, VideoSettings,
};
use medea_macro::enum_delegate;

use crate::{
    api::control::{
        endpoints::webrtc_publish_endpoint::PublishPolicy, MemberId,
    },
    media::{IceUser, MediaTrack},
    signalling::{
        elements::endpoints::{
            webrtc::WebRtcPublishEndpoint, Endpoint, WeakEndpoint,
        },
        peers::Counter,
    },
};

/// Job which will be ran on this [`Peer`] when it will be in [`Stable`] state.
///
/// If [`Peer`] state currently is not [`Stable`] then we should just wait for
/// [`Stable`] state before running this [`Job`].
///
/// After all queued [`Job`]s are executed, renegotiation __should__ be
/// performed.
struct Job(Box<dyn FnOnce(&mut Peer<Stable>)>);

impl Job {
    /// Returns new [`Job`] with provided [`FnOnce`] which will be ran on
    /// [`Job::run`] call.
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce(&mut Peer<Stable>) + 'static,
    {
        Self(Box::new(f))
    }

    /// Calls [`Job`]'s [`FnOnce`] with provided [`Peer`] as parameter.
    pub fn run(self, peer: &mut Peer<Stable>) {
        (self.0)(peer);
    }
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Job").finish()
    }
}

/// Subscriber to the events which indicates that renegotiation process should
/// be started for the some [`Peer`].
#[cfg_attr(test, mockall::automock)]
pub trait RenegotiationSubscriber: fmt::Debug {
    /// Starts renegotiation process for the [`Peer`] with a provided
    /// [`PeerId`].
    ///
    /// Provided [`Peer`] and it's partner [`Peer`] should be in [`Stable`],
    /// otherwise nothing will be done.
    fn renegotiation_needed(&self, peer_id: PeerId);

    /// Returns clone of this [`RenegotiationSubscriber`].
    fn box_clone(&self) -> Box<dyn RenegotiationSubscriber>;
}

#[cfg(test)]
impl fmt::Debug for MockRenegotiationSubscriber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MockRenegotiationSubscriber").finish()
    }
}

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
#[enum_delegate(
    pub fn get_updates(&self) -> Vec<TrackUpdate>
)]
#[enum_delegate(
    pub fn update_senders_statuses(
        &self,
        senders_statuses: HashMap<TrackId, bool>,
    )
)]
#[enum_delegate(pub fn schedule_add_receiver(&mut self, track: Rc<MediaTrack>))]
#[enum_delegate(pub fn schedule_add_sender(&mut self, track: Rc<MediaTrack>))]
#[enum_delegate(pub fn add_endpoint(&mut self, endpoint: &Endpoint))]
#[enum_delegate(
    pub fn add_publisher(
        &mut self,
        src: &WebRtcPublishEndpoint,
        partner_peer: &mut PeerStateMachine,
        tracks_counter: &Counter<TrackId>,
    )
)]
#[derive(Debug)]
pub enum PeerStateMachine {
    WaitLocalSdp(Peer<WaitLocalSdp>),
    WaitLocalHaveRemote(Peer<WaitLocalHaveRemote>),
    WaitRemoteSdp(Peer<WaitRemoteSdp>),
    Stable(Peer<Stable>),
}

impl PeerStateMachine {
    /// Runs [`Job`]s which are scheduled for this [`PeerStateMachine`].
    ///
    /// [`Job`]s will be ran __only if [`Peer`] is in [`Stable`]__ state.
    ///
    /// Returns `true` if at least one [`Job`] was ran.
    ///
    /// Returns `false` if nothing was done.
    pub fn run_scheduled_jobs(&mut self) -> bool {
        if let PeerStateMachine::Stable(stable_peer) = self {
            stable_peer.run_scheduled_jobs()
        } else {
            false
        }
    }

    /// Returns `true` if this [`PeerStateMachine`] currently in [`Stable`]
    /// state.
    pub fn is_stable(&self) -> bool {
        if let PeerStateMachine::Stable(_) = self {
            true
        } else {
            false
        }
    }
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

    /// If `true` then this [`Peer`] is known to client (`Event::PeerCreated`
    /// for this [`Peer`] was sent to the client).
    is_known_to_remote: bool,

    /// Tracks changes, that remote [`Peer`] is not aware of.
    pending_track_updates: Vec<TrackChange>,

    /// Queue of the [`Job`]s which are should be ran when this [`Peer`] will
    /// be [`Stable`].
    ///
    /// [`Job`]s will be ran on [`Peer::renegotiation_finished`] and on
    /// [`Peer::run_scheduled_jobs`] actions.
    ///
    /// When this [`Job`]s will be executed, renegotiation process should be
    /// started for this [`Peer`].
    jobs_queue: VecDeque<Job>,

    /// Subscriber to the events which indicates that renegotiation process
    /// should be started for this [`Peer`].
    renegotiation_subscriber: Box<dyn RenegotiationSubscriber>,
}

/// Tracks changes, that remote [`Peer`] is not aware of.
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
    /// Tries to return [`Track`] based on this [`TrackChange`].
    ///
    /// Returns `None` if this [`TrackChange`] doesn't indicates new [`Track`]
    /// creation.
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
            .map(|change| {
                // TODO: remove this unwrap when new TrackChanges will be
                // implemented.
                change.try_as_track(self.partner_peer_id()).unwrap()
            })
            .map(TrackUpdate::Added)
            .collect()
    }

    /// Returns [`Track`]s that remote [`Peer`] is not aware of.
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

    /// Updates this [`Peer`]'s senders statuses.
    pub fn update_senders_statuses(
        &self,
        senders_statuses: HashMap<TrackId, bool>,
    ) {
        for (track_id, is_publishing) in senders_statuses {
            if let Some(sender) = self.context.senders.get(&track_id) {
                sender.set_enabled(is_publishing);
            }
        }
    }

    /// Returns all receiving [`MediaTrack`]s of this [`Peer`].
    pub fn receivers(&self) -> &HashMap<TrackId, Rc<MediaTrack>> {
        &self.context.receivers
    }

    /// Returns all sending [`MediaTrack`]s of this [`Peer`].
    pub fn senders(&self) -> &HashMap<TrackId, Rc<MediaTrack>> {
        &self.context.senders
    }

    /// If `true` then this [`Peer`] is known to client (`Event::PeerCreated`
    /// for this [`Peer`] was sent to the client).
    pub fn is_known_to_remote(&self) -> bool {
        self.context.is_known_to_remote
    }

    /// Schedules [`Job`] which will be ran before renegotiation process start.
    fn schedule_job(&mut self, job: Job) {
        self.context.jobs_queue.push_back(job);
    }

    /// Schedules [`Track`] adding to [`Peer`] send tracks list.
    ///
    /// This [`Track`] will be considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks` after this scheduled [`Job`] will
    /// be ran.
    fn schedule_add_sender(&mut self, track: Rc<MediaTrack>) {
        self.schedule_job(Job::new(move |peer| {
            peer.context
                .pending_track_updates
                .push(TrackChange::AddSendTrack(Rc::clone(&track)));
            peer.context.senders.insert(track.id, track);
        }))
    }

    /// Schedules [`Track`] adding to [`Peer`] receive tracks list.
    ///
    /// This [`Track`] will be considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks` after this scheduled [`Job`] will
    /// be ran.
    fn schedule_add_receiver(&mut self, track: Rc<MediaTrack>) {
        self.schedule_job(Job::new(move |peer| {
            peer.context
                .pending_track_updates
                .push(TrackChange::AddRecvTrack(Rc::clone(&track)));
            peer.context.receivers.insert(track.id, track);
        }));
    }

    /// Schedules `send` tracks adding to `self` and `recv` tracks for this
    /// `send` to `partner_peer`.
    ///
    /// Actually __nothing will be done__ after this function call. This action
    /// will be ran only before renegotiation start.
    ///
    /// Tracks will be added based on [`WebRtcPublishEndpoint::audio_settings`]
    /// and [`WebRtcPublishEndpoint::video_settings`].
    pub fn add_publisher(
        &mut self,
        src: &WebRtcPublishEndpoint,
        partner_peer: &mut PeerStateMachine,
        tracks_counter: &Counter<TrackId>,
    ) {
        let audio_settings = src.audio_settings();
        if audio_settings.publish_policy != PublishPolicy::Disabled {
            let track_audio = Rc::new(MediaTrack::new(
                tracks_counter.next_id(),
                MediaType::Audio(AudioSettings {
                    is_required: audio_settings.publish_policy.is_required(),
                }),
            ));
            self.schedule_add_sender(Rc::clone(&track_audio));
            partner_peer.schedule_add_receiver(track_audio);
        }

        let video_settings = src.video_settings();
        if video_settings.publish_policy != PublishPolicy::Disabled {
            let track_video = Rc::new(MediaTrack::new(
                tracks_counter.next_id(),
                MediaType::Video(VideoSettings {
                    is_required: video_settings.publish_policy.is_required(),
                }),
            ));
            self.schedule_add_sender(Rc::clone(&track_video));
            partner_peer.schedule_add_receiver(track_video);
        }
    }
}

impl Peer<WaitLocalSdp> {
    /// Sets local description and transition [`Peer`] to [`WaitRemoteSdp`]
    /// state.
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
        self.context.sdp_answer = Some(sdp_answer.to_string());

        let mut peer = Peer {
            context: self.context,
            state: Stable {},
        };
        peer.renegotiation_finished();

        peer
    }
}

impl Peer<WaitLocalHaveRemote> {
    /// Sets local description and transitions [`Peer`] to [`Stable`] state.
    pub fn set_local_sdp(mut self, sdp_answer: String) -> Peer<Stable> {
        self.context.sdp_answer = Some(sdp_answer);

        let mut peer = Peer {
            context: self.context,
            state: Stable {},
        };
        peer.renegotiation_finished();

        peer
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
        renegotiation_subscriber: Box<dyn RenegotiationSubscriber>,
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
            is_known_to_remote: false,
            pending_track_updates: Vec::new(),
            jobs_queue: VecDeque::new(),
            renegotiation_subscriber,
        };

        Self {
            context,
            state: Stable {},
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
        context.sdp_answer = None;
        context.sdp_offer = None;

        Peer {
            context,
            state: WaitLocalSdp {},
        }
    }

    /// Runs [`Job`]s which are scheduled for this [`Peer`].
    ///
    /// Returns `true` if at least one [`Job`] was ran.
    ///
    /// Returns `false` if nothing was done.
    fn run_scheduled_jobs(&mut self) -> bool {
        if self.context.jobs_queue.is_empty() {
            false
        } else {
            while let Some(job) = self.context.jobs_queue.pop_back() {
                job.run(self);
            }

            self.context
                .renegotiation_subscriber
                .renegotiation_needed(self.id());

            true
        }
    }

    /// Sets [`Context::is_known_to_remote`] to `true`.
    ///
    /// Resets [`Context::pending_track_updates`] buffer.
    ///
    /// Runs all scheduled [`Job`]s of this [`Peer`].
    ///
    /// Should be called when renegotiation was finished.
    fn renegotiation_finished(&mut self) {
        self.context.is_known_to_remote = true;
        self.context.pending_track_updates.clear();
        self.run_scheduled_jobs();
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Returns dummy [`RenegotiationSubscriber`] mock which does nothing.
    pub fn dummy_renegotiation_sub_mock() -> Box<dyn RenegotiationSubscriber> {
        let mut mock = MockRenegotiationSubscriber::new();
        mock.expect_renegotiation_needed().returning(|_| ());
        mock.expect_box_clone()
            .returning(|| dummy_renegotiation_sub_mock());

        Box::new(mock)
    }

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
            dummy_renegotiation_sub_mock(),
        );

        let track_id_counter = Counter::default();

        for _ in 0..send_audio {
            let track_id = track_id_counter.next_id();
            let track = MediaTrack::new(
                track_id,
                MediaType::Audio(AudioSettings { is_required: true }),
            );
            peer.context.senders.insert(track_id, Rc::new(track));
        }

        for _ in 0..send_video {
            let track_id = track_id_counter.next_id();
            let track = MediaTrack::new(
                track_id,
                MediaType::Video(VideoSettings { is_required: true }),
            );
            peer.context.senders.insert(track_id, Rc::new(track));
        }

        for _ in 0..recv_audio {
            let track_id = track_id_counter.next_id();
            let track = MediaTrack::new(
                track_id,
                MediaType::Audio(AudioSettings { is_required: true }),
            );
            peer.context.receivers.insert(track_id, Rc::new(track));
        }

        for _ in 0..recv_video {
            let track_id = track_id_counter.next_id();
            let track = MediaTrack::new(
                track_id,
                MediaType::Video(VideoSettings { is_required: true }),
            );
            peer.context.receivers.insert(track_id, Rc::new(track));
        }

        peer.into()
    }

    fn media_track(track_id: u64) -> Rc<MediaTrack> {
        Rc::new(MediaTrack::new(
            TrackId(track_id),
            MediaType::Video(VideoSettings { is_required: true }),
        ))
    }

    #[test]
    fn scheduled_tasks_normally_ran() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut renegotiation_sub = MockRenegotiationSubscriber::new();
        renegotiation_sub.expect_renegotiation_needed().returning(
            move |peer_id| {
                tx.send(peer_id).unwrap();
            },
        );

        let mut peer = Peer::new(
            PeerId(0),
            MemberId("member-1".to_string()),
            PeerId(1),
            MemberId("member-2".to_string()),
            false,
            Box::new(renegotiation_sub),
        );

        peer.schedule_add_receiver(media_track(0));
        peer.schedule_add_sender(media_track(1));

        assert!(peer.context.senders.is_empty());
        assert!(peer.context.receivers.is_empty());

        assert!(peer.run_scheduled_jobs());
        assert_eq!(rx.recv().unwrap(), PeerId(0));

        assert_eq!(peer.context.senders.len(), 1);
        assert_eq!(peer.context.receivers.len(), 1);
    }

    #[test]
    fn scheduled_tasks_will_be_ran_on_stable() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut renegotiation_sub = MockRenegotiationSubscriber::new();
        renegotiation_sub.expect_renegotiation_needed().returning(
            move |peer_id| {
                tx.send(peer_id).unwrap();
            },
        );

        let peer = Peer::new(
            PeerId(0),
            MemberId("member-1".to_string()),
            PeerId(1),
            MemberId("member-2".to_string()),
            false,
            Box::new(renegotiation_sub),
        );

        let mut peer = peer.start();
        peer.schedule_add_sender(media_track(0));
        peer.schedule_add_receiver(media_track(1));
        assert!(peer.context.senders.is_empty());
        assert!(peer.context.receivers.is_empty());

        let peer = peer.set_local_sdp(String::new());
        assert!(peer.context.senders.is_empty());
        assert!(peer.context.receivers.is_empty());

        let peer = peer.set_remote_sdp("");
        assert_eq!(peer.context.receivers.len(), 1);
        assert_eq!(peer.context.senders.len(), 1);
        assert_eq!(peer.context.pending_track_updates.len(), 2);
        assert_eq!(peer.context.jobs_queue.len(), 0);
        assert_eq!(rx.recv().unwrap(), PeerId(0));
    }
}
