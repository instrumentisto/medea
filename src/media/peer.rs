//! Remote [`RTCPeerConnection`][1] representation.
//!
//! Some changes requires (re)negotiation, and they should be firstly scheduled.
//! And if (re)negotiation process currently is in progress, new (re)negotiation
//! can't be done and should wait until going (re)negotiation doesn't finished.
//!
//! When you're scheduled some [`TrackChange`], you should try to run they with
//! [`PeerStateMachine::commit_scheduled_tasks`]. If [`Peer`] currently is not
//! in [`Stable`] state then nothing will be done. And only when [`Peer`] will
//! be transferred into [`Stable`] state, then scheduled [`Task`]s will be
//! executed and new (re)negotiation process will be started automatically by
//! calling [`NegotiationSubscriber::negotiation_needed`].
//!
//! All functions from the [`PeerChangesScheduler`] requires (re)negotiation and
//! will be ran only by calling [`PeerStateMachine::commit_scheduler_tasks`] or
//! on [`Peer`] transferring to the [`Stable`] state.
//!
//! # How to perform update which requires (re)negotiation
//!
//! If you wanna perform update which requires (re)negotiation:
//!
//! 1. Schedule this change (call any function of the [`PeerChangesScheduler`]
//! for example)
//!
//! 2. Optionally, you can schedule as many changes as you want
//!
//! 3. Call [`PeerStateMachine::commit_scheduled_tasks`]
//!
//! Even if [`Peer`] not in [`Stable`] state, you can don't care about it.
//! [`Peer`] will automatically run all scheduled [`Task`]s and start
//! (re)negotiation process by itself when will be transferred into [`Stable`]
//! state.
//!
//! # Implementing [`Peer`] update which requires (re)negotiation
//!
//! 1. All changes which are requires (re)negotiation - should be done by adding
//!    new variant into [`TrackChange`]
//!
//! 2. Implement your changing logic in the [`TrackChangeHandler`]
//!    implementation
//!
//! 3. Create function in the [`PeerChangesScheduler`] which will schedule your
//!    change by adding it into [`Context::scheduled_tasks`]
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
    Track, TrackId, TrackPatch, TrackUpdate, VideoSettings,
};
use medea_macro::{dispatchable, enum_delegate};

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

use self::peer_mutation_task::Task;

/// Subscriber to the events which indicates that negotiation process should
/// be started for the some [`Peer`].
#[cfg_attr(test, mockall::automock)]
pub trait NegotiationSubscriber: fmt::Debug {
    /// Starts negotiation process for the [`Peer`] with a provided
    /// [`PeerId`].
    ///
    /// Provided [`Peer`] and it's partner [`Peer`] should be in [`Stable`],
    /// otherwise nothing will be done.
    fn negotiation_needed(&self, peer_id: PeerId);
}

#[cfg(test)]
impl fmt::Debug for MockNegotiationSubscriber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MockNegotiationSubscriber").finish()
    }
}

/// [`Peer`] doesn't have remote [SDP] and is waiting for local [SDP].
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

/// No negotiation happening atm. It may have been ended or haven't yet started.
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

/// Scheduler of the changes of the [`Peer`] which are requires (re)negotiation.
pub struct PeerChangesScheduler<'a> {
    /// [`Context`] of the [`Peer`] in which will scheduled changes.
    context: &'a mut Context,
}

impl<'a> PeerChangesScheduler<'a> {
    /// Schedules [`Task`] which will be ran before negotiation process start.
    fn schedule_task(&mut self, job: Task) {
        self.context.tasks_queue.push_back(job);
    }

    /// Schedules [`Track`] adding to [`Peer`] receive tracks list.
    ///
    /// This [`Track`] will be considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks` after this scheduled [`Task`] will
    /// be ran.
    pub fn add_receiver(&mut self, track: Rc<MediaTrack>) {
        self.schedule_task(Task::new(TrackChange::AddRecvTrack(track)));
    }

    /// Schedules [`Track`] adding to [`Peer`] send tracks list.
    ///
    /// This [`Track`] will be considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks` after this scheduled [`Task`] will
    /// be ran.
    pub fn add_sender(&mut self, track: Rc<MediaTrack>) {
        self.schedule_task(Task::new(TrackChange::AddSendTrack(track)));
    }

    /// Schedules provided [`TrackPatch`]s.
    ///
    /// Provided [`TrackPatch`]s will be sent to the client on (re)negotiation.
    pub fn patch_tracks(&mut self, patches: Vec<TrackPatch>) {
        for patch in patches {
            self.schedule_task(Task::new(TrackChange::TrackPatch(patch)));
        }
    }

    /// Schedules `send` tracks adding to `self` and `recv` tracks for this
    /// `send` to `partner_peer`.
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
            self.add_sender(Rc::clone(&track_audio));
            partner_peer
                .as_changes_scheduler()
                .add_receiver(track_audio);
        }

        let video_settings = src.video_settings();
        if video_settings.publish_policy != PublishPolicy::Disabled {
            let track_video = Rc::new(MediaTrack::new(
                tracks_counter.next_id(),
                MediaType::Video(VideoSettings {
                    is_required: video_settings.publish_policy.is_required(),
                }),
            ));
            self.add_sender(Rc::clone(&track_video));
            partner_peer
                .as_changes_scheduler()
                .add_receiver(track_video);
        }
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
#[enum_delegate(pub fn endpoints(&self) -> Vec<WeakEndpoint>)]
#[enum_delegate(pub fn add_endpoint(&mut self, endpoint: &Endpoint))]
#[enum_delegate(
    pub fn receivers(&self) -> &HashMap<TrackId, Rc<MediaTrack>>
)]
#[enum_delegate(pub fn senders(&self) -> &HashMap<TrackId, Rc<MediaTrack>>)]
#[enum_delegate(
    pub fn get_updates(&self) -> Vec<TrackUpdate>
)]
#[enum_delegate(
    pub fn update_senders_statuses(
        &self,
        senders_statuses: HashMap<TrackId, bool>,
    )
)]
#[enum_delegate(pub fn as_changes_scheduler(&mut self) -> PeerChangesScheduler)]
#[derive(Debug)]
pub enum PeerStateMachine {
    WaitLocalSdp(Peer<WaitLocalSdp>),
    WaitLocalHaveRemote(Peer<WaitLocalHaveRemote>),
    WaitRemoteSdp(Peer<WaitRemoteSdp>),
    Stable(Peer<Stable>),
}

impl PeerStateMachine {
    /// Runs [`Task`]s which are scheduled for this [`PeerStateMachine`].
    ///
    /// [`Task`]s will be ran __only if [`Peer`] is in [`Stable`]__ state.
    ///
    /// Returns `true` if at least one [`Task`] was ran.
    ///
    /// Returns `false` if nothing was done.
    pub fn commit_scheduled_changes(&mut self) -> bool {
        if let PeerStateMachine::Stable(stable_peer) = self {
            stable_peer.commit_scheduled_changes()
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

    /// Indicator whether this [`Peer`] must be forcibly connected through
    /// TURN.
    is_force_relayed: bool,

    /// Weak references to the [`Endpoint`]s related to this [`Peer`].
    endpoints: Vec<WeakEndpoint>,

    /// Indicator whether this [`Peer`] was created on remote.
    is_known_to_remote: bool,

    /// Tracks changes, that remote [`Peer`] is not aware of.
    pending_track_updates: Vec<TrackChange>,

    /// Queue of the [`Task`]s which are should be ran when this [`Peer`] will
    /// be [`Stable`].
    ///
    /// [`Task`]s will be ran on [`Peer::negotiation_finished`] and on
    /// [`Peer::commit_scheduled_changes`] actions.
    ///
    /// When this [`Task`]s will be executed, negotiation process should be
    /// started for this [`Peer`].
    tasks_queue: VecDeque<Task>,

    /// Subscriber to the events which indicates that negotiation process
    /// should be started for this [`Peer`].
    negotiation_subscriber: Rc<dyn NegotiationSubscriber>,
}

/// Tracks changes, that remote [`Peer`] is not aware of.
#[dispatchable]
#[derive(Clone, Debug)]
enum TrackChange {
    /// [`MediaTrack`]s with [`Direction::Send`] of this [`Peer`] that remote
    /// Peer is not aware of.
    AddSendTrack(Rc<MediaTrack>),

    /// [`MediaTrack`]s with [`Direction::Recv`] of this [`Peer`] that remote
    /// Peer is not aware of.
    AddRecvTrack(Rc<MediaTrack>),

    /// Changes to some [`MediaTrack`], that remote Peer is not aware of.
    TrackPatch(TrackPatch),
}

impl TrackChange {
    /// Tries to return new [`Track`] based on this [`TrackChange`].
    ///
    /// Returns `None` if this [`TrackChange`] doesn't indicates new [`Track`]
    /// creation.
    fn as_new_track(&self, partner_peer_id: Id) -> Option<Track> {
        match self.as_track_update(partner_peer_id) {
            TrackUpdate::Added(track) => Some(track),
            _ => None,
        }
    }

    /// Returns [`TrackUpdate`] based on this [`TrackChange`].
    fn as_track_update(&self, partner_peer_id: Id) -> TrackUpdate {
        match self {
            TrackChange::AddSendTrack(track) => TrackUpdate::Added(Track {
                id: track.id,
                media_type: track.media_type.clone(),
                direction: Direction::Send {
                    receivers: vec![partner_peer_id],
                    mid: track.mid(),
                },
            }),
            TrackChange::AddRecvTrack(track) => TrackUpdate::Added(Track {
                id: track.id,
                media_type: track.media_type.clone(),
                direction: Direction::Recv {
                    sender: partner_peer_id,
                    mid: track.mid(),
                },
            }),
            TrackChange::TrackPatch(track_patch) => {
                TrackUpdate::Updated(track_patch.clone())
            }
        }
    }
}

impl TrackChangeHandler for Peer<Stable> {
    type Output = ();

    /// Inserts provided [`MediaTrack`] into [`Context::senders`].
    fn on_add_send_track(&mut self, track: Rc<MediaTrack>) {
        self.context.senders.insert(track.id, track);
    }

    /// Inserts provided [`MediaTrack`] into [`Context::receivers`].
    fn on_add_recv_track(&mut self, track: Rc<MediaTrack>) {
        self.context.receivers.insert(track.id, track);
    }

    /// Does nothing.
    fn on_track_patch(&mut self, _: TrackPatch) {}
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
            .map(|c| c.as_track_update(self.partner_peer_id()))
            .collect()
    }

    /// Returns [`Track`]s that remote [`Peer`] is not aware of.
    pub fn new_tracks(&self) -> Vec<Track> {
        self.context
            .pending_track_updates
            .iter()
            .filter_map(|c| c.as_new_track(self.partner_peer_id()))
            .collect()
    }

    /// Indicates whether this [`Peer`] has any send tracks.
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

    /// Indicates whether this [`Peer`] is known to client (`Event::PeerCreated`
    /// for this [`Peer`] was sent to the client).
    pub fn is_known_to_remote(&self) -> bool {
        self.context.is_known_to_remote
    }

    /// Returns [`PeerChangesScheduler`] for this [`Peer`].
    pub fn as_changes_scheduler(&mut self) -> PeerChangesScheduler {
        PeerChangesScheduler {
            context: &mut self.context,
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
    /// [mid]: https://developer.mozilla.org/docs/Web/API/RTCRtpTransceiver/mid
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
        peer.negotiation_finished();

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
        peer.negotiation_finished();

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
        negotiation_subscriber: Rc<dyn NegotiationSubscriber>,
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
            tasks_queue: VecDeque::new(),
            negotiation_subscriber,
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
    pub fn start_negotiation(self) -> Peer<WaitLocalSdp> {
        let mut context = self.context;
        context.sdp_answer = None;
        context.sdp_offer = None;

        Peer {
            context,
            state: WaitLocalSdp {},
        }
    }

    /// Runs [`Task`]s which are scheduled for this [`Peer`].
    ///
    /// Returns `true` if at least one [`Task`] was ran.
    ///
    /// Returns `false` if nothing was done.
    fn commit_scheduled_changes(&mut self) -> bool {
        if self.context.tasks_queue.is_empty() {
            false
        } else {
            while let Some(task) = self.context.tasks_queue.pop_front() {
                task.run(self);
            }

            self.context
                .negotiation_subscriber
                .negotiation_needed(self.id());

            true
        }
    }

    /// Sets [`Context::is_known_to_remote`] to `true`.
    ///
    /// Resets [`Context::pending_track_updates`] buffer.
    ///
    /// Runs all scheduled [`Task`]s of this [`Peer`].
    ///
    /// Should be called when negotiation was finished.
    fn negotiation_finished(&mut self) {
        self.context.is_known_to_remote = true;
        self.context.pending_track_updates.clear();
        self.commit_scheduled_changes();
    }
}

mod peer_mutation_task {
    use std::fmt;

    use crate::media::peer::TrackChange;

    use super::{Peer, Stable};

    /// Job which will be ran on this [`Peer`] when it will be in [`Stable`]
    /// state.
    ///
    /// If [`Peer`] state currently is not [`Stable`] then we should just wait
    /// for [`Stable`] state before running this [`Task`].
    ///
    /// After all queued [`Task`]s are executed, negotiation __should__ be
    /// performed.
    pub(super) struct Task(TrackChange);

    impl Task {
        /// Returns new [`Task`] with provided [`FnOnce`] which will be ran on
        /// [`Task::run`] call.
        pub fn new(change: TrackChange) -> Self {
            Self(change)
        }

        /// Calls [`Task`]'s [`FnOnce`] with provided [`Peer`] as parameter.
        pub fn run(self, peer: &mut Peer<Stable>) {
            peer.context.pending_track_updates.push(self.0.clone());
            self.0.dispatch_with(peer);
        }
    }

    impl fmt::Debug for Task {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_tuple("Task").finish()
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Returns dummy [`NegotiationSubscriber`] mock which does nothing.
    pub fn dummy_negotiation_sub_mock() -> Rc<dyn NegotiationSubscriber> {
        let mut mock = MockNegotiationSubscriber::new();
        mock.expect_negotiation_needed().returning(|_| ());

        Rc::new(mock)
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
            dummy_negotiation_sub_mock(),
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

    fn media_track(track_id: u32) -> Rc<MediaTrack> {
        Rc::new(MediaTrack::new(
            TrackId(track_id),
            MediaType::Video(VideoSettings { is_required: true }),
        ))
    }

    #[test]
    fn scheduled_tasks_normally_ran() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut negotiation_sub = MockNegotiationSubscriber::new();
        negotiation_sub
            .expect_negotiation_needed()
            .returning(move |peer_id| {
                tx.send(peer_id).unwrap();
            });

        let mut peer = Peer::new(
            PeerId(0),
            MemberId("member-1".to_string()),
            PeerId(1),
            MemberId("member-2".to_string()),
            false,
            Rc::new(negotiation_sub),
        );

        peer.as_changes_scheduler().add_receiver(media_track(0));
        peer.as_changes_scheduler().add_sender(media_track(1));

        assert!(peer.context.senders.is_empty());
        assert!(peer.context.receivers.is_empty());

        assert!(peer.commit_scheduled_changes());
        assert_eq!(rx.recv().unwrap(), PeerId(0));

        assert_eq!(peer.context.senders.len(), 1);
        assert_eq!(peer.context.receivers.len(), 1);
    }

    #[test]
    fn scheduled_tasks_will_be_ran_on_stable() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut negotiation_sub = MockNegotiationSubscriber::new();
        negotiation_sub
            .expect_negotiation_needed()
            .returning(move |peer_id| {
                tx.send(peer_id).unwrap();
            });

        let peer = Peer::new(
            PeerId(0),
            MemberId("member-1".to_string()),
            PeerId(1),
            MemberId("member-2".to_string()),
            false,
            Rc::new(negotiation_sub),
        );

        let mut peer = peer.start();
        peer.as_changes_scheduler().add_sender(media_track(0));
        peer.as_changes_scheduler().add_receiver(media_track(1));
        assert!(peer.context.senders.is_empty());
        assert!(peer.context.receivers.is_empty());

        let peer = peer.set_local_sdp(String::new());
        assert!(peer.context.senders.is_empty());
        assert!(peer.context.receivers.is_empty());

        let peer = peer.set_remote_sdp("");
        assert_eq!(peer.context.receivers.len(), 1);
        assert_eq!(peer.context.senders.len(), 1);
        assert_eq!(peer.context.pending_track_updates.len(), 2);
        assert_eq!(peer.context.tasks_queue.len(), 0);
        assert_eq!(rx.recv().unwrap(), PeerId(0));
    }
}
