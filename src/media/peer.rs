//! Remote [`RTCPeerConnection`][1] representation.
//!
//! # Applying changes to [`Peer`]
//!
//! Some [`Peer`] state changes require SDP negotiation. SDP negotiation is a
//! process that requires some message exchange between remote `Peer`s, so it
//! cannot be performed immediately in a place.
//!
//! The problem arises when we need to apply changes to [`Peer`] while it's
//! already performing negotiation caused by another changes. In this case we
//! cannot start a new negotiation and should wait until ongoing negotiation
//! is finished.
//!
//! So, how [`PeerStateMachine`] handles such situations?
//!
//! All methods performing changes that might require negotiations are placed in
//! a [`PeerChangesScheduler`], which can be obtained via
//! [`PeerStateMachine::as_changes_scheduler`].
//!
//! Calling [`PeerChangesScheduler`] methods don't change the [`Peer`]'s actual
//! state, but just schedules those changes to be applied when it will be
//! appropriate.
//!
//! After scheduling changes you should call
//! [`PeerStateMachine::commit_scheduled_changes`], which will try to apply
//! changes, but if the [`Peer`] is not in a [`Stable`] state then it's no-op,
//! and these changes will be applied when the [`Peer`] will be transferred into
//! a [`Stable`] state only.
//!
//! After the changes are applied, the [`Peer`] will notify
//! [`PeerUpdatesSubscriber`] that it's appropriate to start a negotiation.
//!
//! # Implementing [`Peer`]'s update that requires (re)negotiation
//!
//! 1. All changes that require (re)negotiation should be done by adding a new
//!    variant into [`TrackChange`].
//! 2. Implement your changing logic in the [`TrackChangeHandler`]
//!    implementation.
//! 3. Create a function in the [`PeerChangesScheduler`] which will schedule
//!    your change by adding it into the [`Context::track_changes_queue`].
//!
//! # Force [`Peer`] updates which are requires renegotiation
//!
//! Some updates are require renegotiation, but at same time they require
//! instant [`Event::TracksApplied`] response even if [`Peer`]s will be out
//! of sync for a while.
//!
//! ## Usage
//!
//! If you wanna just use already implemented forcible [`TrackChange`] - use it
//! same as non-forcible changes. [`Peer`] implementation will take care of how
//! to do it correctly. __No extra actions are required.__
//!
//! ## Algorithm of the forcible [`Peer`] update
//!
//! ### If [`Peer`] isn't [`Stable`]
//!
//! 1. Send [`Event::TracksApplied`] with `renegotiation_role: None`
//! 2. When [`Peer`] goes into [`Stable`] state, start new renegotiation process
//!
//! ### If [`Peer`] is [`Stable`]
//!
//! Same as non-forcible [`TrackChange`].
//!
//! ## Implementation
//!
//! 1. Implement your change same as non-forcible [`TrackChange`]
//! 2. Return `true` for your [`TrackChange`] variant in the
//!    [`TrackChange::is_forcible`] function
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
    AudioSettings, Direction, IceServer, MediaType, MemberId, PeerId as Id,
    PeerId, Track, TrackId, TrackPatch, TrackUpdate, VideoSettings,
};
use medea_macro::{dispatchable, enum_delegate};

use crate::{
    api::control::endpoints::webrtc_publish_endpoint::PublishPolicy,
    media::{IceUser, MediaTrack},
    signalling::{
        elements::endpoints::{
            webrtc::WebRtcPublishEndpoint, Endpoint, WeakEndpoint,
        },
        peers::Counter,
    },
};

/// Subscriber to the events indicating that [`Peer`] was updated.
#[cfg_attr(test, mockall::automock)]
pub trait PeerUpdatesSubscriber: fmt::Debug {
    /// Starts negotiation process for the [`Peer`] with the provided `peer_id`.
    ///
    /// Provided [`Peer`] and it's partner [`Peer`] should be in a [`Stable`]
    /// state, otherwise only forced [`TrackChange`]s will be sent.
    fn negotiation_needed(&self, peer_id: PeerId);

    /// Forcebly updates [`Peer`] without renegotiation.
    fn force_update(&self, peer_id: PeerId, changes: Vec<TrackUpdate>);
}

#[cfg(test)]
impl fmt::Debug for MockPeerUpdatesSubscriber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MockPeerUpdatesSubscriber").finish()
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
#[enum_delegate(pub fn commit_forcible_changes(&mut self))]
#[derive(Debug)]
pub enum PeerStateMachine {
    WaitLocalSdp(Peer<WaitLocalSdp>),
    WaitLocalHaveRemote(Peer<WaitLocalHaveRemote>),
    WaitRemoteSdp(Peer<WaitRemoteSdp>),
    Stable(Peer<Stable>),
}

impl PeerStateMachine {
    /// Tries to run all scheduled changes.
    ///
    /// Changes are applied __only if [`Peer`] is in a [`Stable`]__ state.
    #[inline]
    pub fn commit_scheduled_changes(&mut self) {
        if let PeerStateMachine::Stable(stable_peer) = self {
            stable_peer.commit_scheduled_changes();
        } else {
            self.commit_forcible_changes();
        }
    }

    /// Returns `true` if this [`PeerStateMachine`] currently in [`Stable`]
    /// state.
    #[inline]
    #[must_use]
    pub fn is_stable(&self) -> bool {
        matches!(self, PeerStateMachine::Stable(_))
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

    /// Queue of the [`TrackChange`]s that are scheduled to apply when this
    /// [`Peer`] will be in a [`Stable`] state.
    track_changes_queue: VecDeque<TrackChange>,

    /// Subscriber to the events which indicates that negotiation process
    /// should be started for this [`Peer`].
    peer_updates_sub: Rc<dyn PeerUpdatesSubscriber>,

    /// Flag which indicates that this [`Peer`] should be renegotiated when it
    /// will be [`Stable`].
    ///
    /// If this flag `true` then `track_changes_queue` length will be ignored
    /// and renegotiation will be started on any length.
    is_forcibly_updated: bool,
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
    fn as_new_track(&self, partner_member_id: MemberId) -> Option<Track> {
        match self.as_track_update(partner_member_id) {
            TrackUpdate::Added(track) => Some(track),
            TrackUpdate::Updated(_) => None,
        }
    }

    /// Returns [`TrackUpdate`] based on this [`TrackChange`].
    fn as_track_update(&self, partner_member_id: MemberId) -> TrackUpdate {
        match self {
            TrackChange::AddSendTrack(track) => TrackUpdate::Added(Track {
                id: track.id,
                media_type: track.media_type.clone(),
                direction: Direction::Send {
                    receivers: vec![partner_member_id],
                    mid: track.mid(),
                },
            }),
            TrackChange::AddRecvTrack(track) => TrackUpdate::Added(Track {
                id: track.id,
                media_type: track.media_type.clone(),
                direction: Direction::Recv {
                    sender: partner_member_id,
                    mid: track.mid(),
                },
            }),
            TrackChange::TrackPatch(track_patch) => {
                TrackUpdate::Updated(track_patch.clone())
            }
        }
    }

    /// Returns `true` if this [`TrackChange`] can be sent forcibly sent without
    /// instant renegotiation starting.
    pub fn is_forcible(&self) -> bool {
        match self {
            TrackChange::AddSendTrack(_) | TrackChange::AddRecvTrack(_) => {
                false
            }
            TrackChange::TrackPatch(_) => true,
        }
    }
}

impl<T> TrackChangeHandler for Peer<T> {
    type Output = ();

    /// Inserts provided [`MediaTrack`] into [`Context::senders`].
    #[inline]
    fn on_add_send_track(&mut self, track: Rc<MediaTrack>) {
        self.context.senders.insert(track.id, track);
    }

    /// Inserts provided [`MediaTrack`] into [`Context::receivers`].
    #[inline]
    fn on_add_recv_track(&mut self, track: Rc<MediaTrack>) {
        self.context.receivers.insert(track.id, track);
    }

    /// Does nothing.
    #[inline]
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
    #[inline]
    pub fn member_id(&self) -> MemberId {
        self.context.member_id.clone()
    }

    /// Returns ID of [`Peer`].
    #[inline]
    pub fn id(&self) -> Id {
        self.context.id
    }

    /// Returns ID of interconnected [`Peer`].
    #[inline]
    pub fn partner_peer_id(&self) -> Id {
        self.context.partner_peer
    }

    /// Returns ID of interconnected [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    #[inline]
    pub fn partner_member_id(&self) -> MemberId {
        self.context.partner_member.clone()
    }

    /// Returns [`TrackUpdate`]s of this [`Peer`] which should be sent to the
    /// client in the [`Event::TracksApplied`].
    pub fn get_updates(&self) -> Vec<TrackUpdate> {
        self.context
            .pending_track_updates
            .iter()
            .map(|c| c.as_track_update(self.partner_member_id()))
            .collect()
    }

    /// Returns [`Track`]s that remote [`Peer`] is not aware of.
    pub fn new_tracks(&self) -> Vec<Track> {
        self.context
            .pending_track_updates
            .iter()
            .filter_map(|c| c.as_new_track(self.partner_member_id()))
            .collect()
    }

    /// Indicates whether this [`Peer`] has any send tracks.
    #[inline]
    pub fn is_sender(&self) -> bool {
        !self.context.senders.is_empty()
    }

    /// Indicates whether all media is forcibly relayed through a TURN server.
    #[inline]
    pub fn is_force_relayed(&self) -> bool {
        self.context.is_force_relayed
    }

    /// Returns vector of [`IceServer`]s built from this [`Peer`]s [`IceUser`].
    #[inline]
    pub fn ice_servers_list(&self) -> Option<Vec<IceServer>> {
        self.context.ice_user.as_ref().map(IceUser::servers_list)
    }

    /// Sets [`IceUser`], which is used to generate [`IceServer`]s
    #[inline]
    pub fn set_ice_user(&mut self, ice_user: IceUser) {
        self.context.ice_user.replace(ice_user);
    }

    /// Returns [`WeakEndpoint`]s for which this [`Peer`] was created.
    #[inline]
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
    #[inline]
    pub fn receivers(&self) -> &HashMap<TrackId, Rc<MediaTrack>> {
        &self.context.receivers
    }

    /// Returns all sending [`MediaTrack`]s of this [`Peer`].
    #[inline]
    pub fn senders(&self) -> &HashMap<TrackId, Rc<MediaTrack>> {
        &self.context.senders
    }

    /// Commits all [`TrackChange`]s which are marked as forcible
    /// ([`TrackChange::is_forcible`]).
    pub fn commit_forcible_changes(&mut self) {
        let track_changes_queue = std::mem::replace(
            &mut self.context.track_changes_queue,
            VecDeque::new(),
        );
        let mut forcible_changes = VecDeque::new();
        let mut filtered_changes_queue = VecDeque::new();
        for track_change in track_changes_queue {
            if track_change.is_forcible() {
                forcible_changes.push_back(track_change);
            } else {
                filtered_changes_queue.push_back(track_change);
            }
        }
        self.context.track_changes_queue = filtered_changes_queue;

        let mut updates = Vec::new();
        for change in forcible_changes {
            let track_update = change.as_track_update(self.partner_member_id());
            change.dispatch_with(self);
            updates.push(track_update);
        }

        if !updates.is_empty() {
            self.context
                .peer_updates_sub
                .force_update(self.id(), updates);
            self.context.is_forcibly_updated = true;
        }
    }

    /// Indicates whether this [`Peer`] is known to client (`Event::PeerCreated`
    /// for this [`Peer`] was sent to the client).
    #[must_use]
    pub fn is_known_to_remote(&self) -> bool {
        self.context.is_known_to_remote
    }

    /// Returns [`PeerChangesScheduler`] for this [`Peer`].
    #[inline]
    #[must_use]
    pub fn as_changes_scheduler(&mut self) -> PeerChangesScheduler {
        PeerChangesScheduler {
            context: &mut self.context,
        }
    }
}

impl Peer<WaitLocalSdp> {
    /// Sets local description and transition [`Peer`] to [`WaitRemoteSdp`]
    /// state.
    #[inline]
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
        negotiation_subscriber: Rc<dyn PeerUpdatesSubscriber>,
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
            track_changes_queue: VecDeque::new(),
            peer_updates_sub: negotiation_subscriber,
            is_forcibly_updated: false,
        };

        Self {
            context,
            state: Stable {},
        }
    }

    /// Transition new [`Peer`] into state of waiting for local description.
    pub fn start(mut self) -> Peer<WaitLocalSdp> {
        self.negotiation_started();

        Peer {
            context: self.context,
            state: WaitLocalSdp {},
        }
    }

    /// Transition new [`Peer`] into state of waiting for remote description.
    pub fn set_remote_sdp(
        mut self,
        sdp_offer: String,
    ) -> Peer<WaitLocalHaveRemote> {
        self.negotiation_started();

        let mut context = self.context;
        context.sdp_offer = Some(sdp_offer);
        Peer {
            context,
            state: WaitLocalHaveRemote {},
        }
    }

    /// This method will be called everytime when [`Peer`] goes from [`Stable`]
    /// state into any other state.
    fn negotiation_started(&mut self) {
        self.context.is_forcibly_updated = false;
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
    pub fn start_negotiation(mut self) -> Peer<WaitLocalSdp> {
        self.negotiation_started();

        let mut context = self.context;
        context.sdp_answer = None;
        context.sdp_offer = None;

        Peer {
            context,
            state: WaitLocalSdp {},
        }
    }

    /// Runs [`TrackChange`]s which are scheduled for this [`Peer`].
    ///
    /// If this [`Peer`] is not [`Stable`] then only forced [`TrackChange`]s
    /// will be ran without renegotiation. Renegotiation for the forced
    /// [`TrackChange`]s will be done when [`Peer`] will be [`Stable`].
    fn commit_scheduled_changes(&mut self) {
        if !self.context.track_changes_queue.is_empty()
            || self.context.is_forcibly_updated
        {
            while let Some(task) = self.context.track_changes_queue.pop_front()
            {
                self.context.pending_track_updates.push(task.clone());
                task.dispatch_with(self);
            }

            self.context.peer_updates_sub.negotiation_needed(self.id());
        }
    }

    /// Sets [`Context::is_known_to_remote`] to `true`.
    ///
    /// Resets [`Context::pending_track_updates`] buffer.
    ///
    /// Applies all scheduled changes.
    ///
    /// Should be called when negotiation was finished.
    fn negotiation_finished(&mut self) {
        self.context.is_known_to_remote = true;
        self.context.pending_track_updates.clear();
        self.commit_scheduled_changes();
    }
}

/// Scheduler of the [`Peer`] state changes that require (re)negotiation.
///
/// Obtainable via `PeerStateMachine::as_changes_scheduler`. Refer to module
/// documentation for more details.
pub struct PeerChangesScheduler<'a> {
    /// [`Context`] of the [`Peer`] in which will scheduled changes.
    context: &'a mut Context,
}

impl<'a> PeerChangesScheduler<'a> {
    /// Schedules provided [`TrackPatch`]s.
    ///
    /// Provided [`TrackPatch`]s will be sent to the client on (re)negotiation.
    pub fn patch_tracks(&mut self, patches: Vec<TrackPatch>) {
        for patch in patches {
            self.schedule_change(TrackChange::TrackPatch(patch));
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

    /// Adds provided [`TrackChange`] to scheduled changes queue.
    #[inline]
    fn schedule_change(&mut self, job: TrackChange) {
        self.context.track_changes_queue.push_back(job);
    }

    /// Schedules [`Track`] addition to [`Peer`] receive tracks list.
    ///
    /// This [`Track`] will be considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks` after this scheduled
    /// [`TrackChange`] will be applied.
    #[inline]
    fn add_receiver(&mut self, track: Rc<MediaTrack>) {
        self.schedule_change(TrackChange::AddRecvTrack(track));
    }

    /// Schedules [`Track`] addition to [`Peer`] send tracks list.
    ///
    /// This [`Track`] will be considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks` after this scheduled
    /// [`TrackChange`] will be applied.
    #[inline]
    fn add_sender(&mut self, track: Rc<MediaTrack>) {
        self.schedule_change(TrackChange::AddSendTrack(track));
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Returns dummy [`PeerUpdatesSubscriber`] mock which does nothing.
    pub fn dummy_negotiation_sub_mock() -> Rc<dyn PeerUpdatesSubscriber> {
        let mut mock = MockPeerUpdatesSubscriber::new();
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
    fn scheduled_changes_normally_ran() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
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

        peer.commit_scheduled_changes();

        assert_eq!(rx.recv().unwrap(), PeerId(0));
        assert_eq!(peer.context.senders.len(), 1);
        assert_eq!(peer.context.receivers.len(), 1);
    }

    #[test]
    fn scheduled_changes_will_be_ran_on_stable() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
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
        assert_eq!(peer.context.track_changes_queue.len(), 0);
        assert_eq!(rx.recv().unwrap(), PeerId(0));
    }

    #[test]
    fn force_updates_works() {
        let (force_update_tx, force_update_rx) = std::sync::mpsc::channel();
        let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
        negotiation_sub.expect_force_update().returning(
            move |peer_id: PeerId, changes: Vec<TrackUpdate>| {
                force_update_tx.send((peer_id, changes)).unwrap();
            },
        );
        let (negotiation_needed_tx, negotiation_needed_rx) =
            std::sync::mpsc::channel();
        negotiation_sub.expect_negotiation_needed().returning(
            move |peer_id: PeerId| {
                negotiation_needed_tx.send(peer_id).unwrap();
            },
        );

        let mut peer = Peer::new(
            PeerId(0),
            MemberId("member-1".to_string()),
            PeerId(1),
            MemberId("member-2".to_string()),
            false,
            Rc::new(negotiation_sub),
        );
        peer.as_changes_scheduler().add_sender(media_track(0));
        peer.as_changes_scheduler().add_receiver(media_track(1));
        peer.commit_scheduled_changes();
        let mut peer = peer.start();

        peer.as_changes_scheduler().patch_tracks(vec![
            TrackPatch {
                id: TrackId(0),
                is_muted: Some(true),
            },
            TrackPatch {
                id: TrackId(1),
                is_muted: Some(true),
            },
        ]);
        peer.commit_forcible_changes();
        let (peer_id, changes) = force_update_rx.recv().unwrap();

        assert_eq!(peer_id, PeerId(0));
        assert_eq!(changes.len(), 2);
        assert!(peer.context.track_changes_queue.is_empty());

        let peer = peer.set_local_sdp(String::new());
        peer.set_remote_sdp("");

        let peer_id = negotiation_needed_rx.recv().unwrap();
        assert_eq!(peer_id, PeerId(0));
    }
}
