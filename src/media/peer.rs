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
//! # Applying changes regardless of [`Peer`] state
//!
//! Sometimes you may want to apply changes immediately, and perform
//! renegotiation later. In this case you should call
//! [`PeerStateMachine::force_commit_scheduled_changes`]`.
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
use std::collections::HashSet;

/// Subscriber to the events indicating that [`Peer`] was updated.
#[cfg_attr(test, mockall::automock)]
pub trait PeerUpdatesSubscriber: fmt::Debug {
    /// Notifies subscriber that provided [`Peer`] must be negotiated.
    fn negotiation_needed(&self, peer_id: PeerId);

    /// Notifies subscriber that provided [`TrackUpdate`] were forcibly (without
    /// negotiation) applied to [`Peer`].
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

/// Implementation of [`Peer`] state machine.
///
/// # State transitions scheme
///
/// ```text
/// +---------------+                   +-----------------+
/// |               |  set_local_offer  |                 |
/// | WaitLocalSdp  +------------------>+  WaitRemoteSdp  |
/// |               |                   |                 |
/// +------+--------+                   +--------+--------+
///        ^                                     |
///        |                                     |
///        |                                     |
/// start_as_offerer                       set_remote_answer
///        |                                     |
///        |                                     |
/// +------+--------+                            |
/// |               +<---------------------------+
/// |   Stable      |
/// |               +<---------------------------+
/// +------+--------+                            |
///        |                                     |
///        |                                     |
/// start_as_answerer                      set_local_answer
///        |                                     |
///        |                                     |
///        v                                     |
/// +------+--------+                   +--------+---------+
/// |               | set_remote_offer  |                  |
/// | WaitRemoteSdp +------------------>+   WaitLocalSdp   |
/// |               |                   |                  |
/// +---------------+                   +------------------+
/// ```
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
#[enum_delegate(pub fn as_changes_scheduler(&mut self) -> PeerChangesScheduler)]
#[enum_delegate(fn inner_force_commit_scheduled_changes(&mut self))]
#[derive(Debug)]
pub enum PeerStateMachine {
    WaitLocalSdp(Peer<WaitLocalSdp>),
    WaitRemoteSdp(Peer<WaitRemoteSdp>),
    Stable(Peer<Stable>),
}

impl PeerStateMachine {
    /// Tries to run all scheduled changes.
    ///
    /// Changes are applied __only if [`Peer`] is in a [`Stable`]__ state.
    #[inline]
    pub fn commit_scheduled_changes(&mut self) -> bool {
        if let PeerStateMachine::Stable(this) = self {
            this.commit_scheduled_changes();
            true
        } else {
            false
        }
    }

    /// Runs scheduled changes if [`Peer`] is in [`Stable`] state, otherwise,
    /// runs scheduled changes forcibly (not all changes can be ran forcibly).
    #[inline]
    pub fn force_commit_scheduled_changes(&mut self) {
        if !self.commit_scheduled_changes() {
            self.inner_force_commit_scheduled_changes();
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

    /// Returns `true` if this [`TrackChange`] can be forcibly applied.
    fn can_force_apply(&self) -> bool {
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

struct TrackPatchDeduper {
    merged_patches: HashMap<TrackId, TrackPatch>,
    whitelist: Option<HashSet<TrackId>>,
}

impl TrackPatchDeduper {
    pub fn new() -> Self {
        Self {
            merged_patches: HashMap::new(),
            whitelist: None,
        }
    }

    pub fn with_whitelist(whitelist: HashSet<TrackId>) -> Self {
        Self {
            merged_patches: HashMap::new(),
            whitelist: Some(whitelist),
        }
    }

    fn filter_patch<'a>(
        &self,
        change: &'a TrackChange,
    ) -> Option<&'a TrackPatch> {
        if !change.can_force_apply() {
            return None;
        }

        if let TrackChange::TrackPatch(patch) = change {
            if let Some(whitelist) = &self.whitelist {
                if whitelist.contains(&patch.id) {
                    Some(patch)
                } else {
                    None
                }
            } else {
                Some(patch)
            }
        } else {
            None
        }
    }

    pub fn merge(&mut self, changes: &mut Vec<TrackChange>) {
        let mut i = 0;
        while i != changes.len() {
            if let Some(patch) = self.filter_patch(&changes[i]) {
                self.merged_patches
                    .entry(patch.id)
                    .or_insert_with(|| TrackPatch::new(patch.id))
                    .merge(patch);

                changes.remove(i);
            } else {
                i += 1;
            };
        }
    }

    pub fn into_track_change_iter(self) -> impl Iterator<Item = TrackChange> {
        self.merged_patches
            .into_iter()
            .map(|(_, patch)| TrackChange::TrackPatch(patch))
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
    /// ([`TrackChange::can_force_apply`]).
    pub fn inner_force_commit_scheduled_changes(&mut self) {
        let track_changes_queue = std::mem::replace(
            &mut self.context.track_changes_queue,
            VecDeque::new(),
        );
        let mut forcible_changes = Vec::new();
        let mut filtered_changes_queue = VecDeque::new();
        for track_change in track_changes_queue {
            if track_change.can_force_apply() {
                forcible_changes.push(track_change);
            } else {
                filtered_changes_queue.push_back(track_change);
            }
        }
        self.context.track_changes_queue = filtered_changes_queue;

        let whitelist = forcible_changes
            .iter()
            .filter_map(|t| {
                if let TrackChange::TrackPatch(patch) = t {
                    Some(patch.id)
                } else {
                    None
                }
            })
            .collect();
        let mut deduper = TrackPatchDeduper::with_whitelist(whitelist);
        deduper.merge(&mut self.context.pending_track_updates);
        deduper.merge(&mut forcible_changes);
        forcible_changes.extend(deduper.into_track_change_iter());

        let mut updates = Vec::new();
        for change in forcible_changes {
            let track_update = change.as_track_update(self.partner_member_id());
            change.dispatch_with(self);
            updates.push(track_update);
        }

        self.dedup_track_changes();

        if !updates.is_empty() {
            self.context
                .peer_updates_sub
                .force_update(self.id(), updates);
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

    /// Deduplicates pending [`TrackChanges`]s.
    fn dedup_track_changes(&mut self) {
        let mut deduper = TrackPatchDeduper::new();
        deduper.merge(&mut self.context.pending_track_updates);

        self.context
            .pending_track_updates
            .extend(deduper.into_track_change_iter());
    }
}

impl Peer<WaitLocalSdp> {
    /// Sets local description and transition [`Peer`] to [`WaitRemoteSdp`]
    /// state.
    #[inline]
    pub fn set_local_offer(self, sdp_offer: String) -> Peer<WaitRemoteSdp> {
        let mut context = self.context;
        context.sdp_offer = Some(sdp_offer);
        Peer {
            context,
            state: WaitRemoteSdp {},
        }
    }

    /// Sets local description and transition [`Peer`] to [`Stable`]
    /// state.
    #[inline]
    pub fn set_local_answer(self, sdp_answer: String) -> Peer<Stable> {
        let mut context = self.context;
        context.sdp_answer = Some(sdp_answer);
        let mut this = Peer {
            context,
            state: Stable {},
        };
        this.negotiation_finished();
        this
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
}

impl Peer<WaitRemoteSdp> {
    /// Sets remote description and transitions [`Peer`] to [`Stable`] state.
    #[inline]
    pub fn set_remote_answer(mut self, sdp_answer: String) -> Peer<Stable> {
        self.context.sdp_answer = Some(sdp_answer);

        let mut peer = Peer {
            context: self.context,
            state: Stable {},
        };
        peer.negotiation_finished();

        peer
    }

    /// Sets remote description and transitions [`Peer`] to [`WaitLocalSdp`]
    /// state.
    #[inline]
    pub fn set_remote_offer(mut self, sdp_offer: String) -> Peer<WaitLocalSdp> {
        self.context.sdp_offer = Some(sdp_offer);

        Peer {
            context: self.context,
            state: WaitLocalSdp {},
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
        };

        Self {
            context,
            state: Stable {},
        }
    }

    /// Changes [`Peer`] state to [`WaitLocalSdp`] and discards previously saved
    /// [SDP] Offer and Answer.
    ///
    /// Sets [`Context::is_renegotiate`] to `true`.
    ///
    /// Resets [`Context::sdp_offer`] and [`Context::sdp_answer`].
    ///
    /// [SDP]: https://tools.ietf.org/html/rfc4317
    #[inline]
    pub fn start_as_offerer(self) -> Peer<WaitLocalSdp> {
        let mut context = self.context;
        context.sdp_answer = None;
        context.sdp_offer = None;

        Peer {
            context,
            state: WaitLocalSdp {},
        }
    }

    /// Changes [`Peer`] state to [`WaitLocalSdp`] and discards previously saved
    /// [SDP] Offer and Answer.
    ///
    /// Sets [`Context::is_renegotiate`] to `true`.
    ///
    /// Resets [`Context::sdp_offer`] and [`Context::sdp_answer`].
    ///
    /// [SDP]: https://tools.ietf.org/html/rfc4317
    #[inline]
    pub fn start_as_answerer(self) -> Peer<WaitRemoteSdp> {
        let mut context = self.context;
        context.sdp_answer = None;
        context.sdp_offer = None;

        Peer {
            context,
            state: WaitRemoteSdp {},
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

    /// Runs [`Task`]s which are scheduled for this [`Peer`].
    fn commit_scheduled_changes(&mut self) {
        if !self.context.track_changes_queue.is_empty() {
            while let Some(task) = self.context.track_changes_queue.pop_front()
            {
                self.context.pending_track_updates.push(task.clone());
                task.dispatch_with(self);
            }

            self.dedup_track_changes();

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

        let mut peer = peer.start_as_offerer();
        peer.as_changes_scheduler().add_sender(media_track(0));
        peer.as_changes_scheduler().add_receiver(media_track(1));
        assert!(peer.context.senders.is_empty());
        assert!(peer.context.receivers.is_empty());

        let peer = peer.set_local_offer(String::new());
        assert!(peer.context.senders.is_empty());
        assert!(peer.context.receivers.is_empty());

        let peer = peer.set_remote_answer(String::new());
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
        let mut peer = peer.start_as_offerer();

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
        peer.inner_force_commit_scheduled_changes();
        let (peer_id, changes) = force_update_rx.recv().unwrap();

        assert_eq!(peer_id, PeerId(0));
        assert_eq!(changes.len(), 2);
        assert!(peer.context.track_changes_queue.is_empty());

        let peer = peer.set_local_offer(String::new());
        peer.set_remote_answer(String::new());

        let peer_id = negotiation_needed_rx.recv().unwrap();
        assert_eq!(peer_id, PeerId(0));
    }

    #[test]
    fn track_patch_dedup_works() {
        let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
        negotiation_sub
            .expect_force_update()
            .returning(move |_: PeerId, _: Vec<TrackUpdate>| {});
        negotiation_sub
            .expect_negotiation_needed()
            .returning(move |_: PeerId| {});
        let mut peer = Peer::new(
            PeerId(0),
            MemberId("member-1".to_string()),
            PeerId(1),
            MemberId("member-2".to_string()),
            false,
            Rc::new(negotiation_sub),
        );

        let patches = vec![
            TrackPatch {
                id: TrackId(1),
                is_muted: Some(true),
            },
            TrackPatch {
                id: TrackId(2),
                is_muted: None,
            },
            TrackPatch {
                id: TrackId(1),
                is_muted: Some(false),
            },
            TrackPatch {
                id: TrackId(2),
                is_muted: Some(true),
            },
            TrackPatch {
                id: TrackId(2),
                is_muted: Some(false),
            },
            TrackPatch {
                id: TrackId(2),
                is_muted: None,
            },
            TrackPatch {
                id: TrackId(1),
                is_muted: None,
            },
        ];
        peer.as_changes_scheduler().patch_tracks(patches);
        let mut peer = PeerStateMachine::from(peer);
        peer.commit_scheduled_changes();
        let peer = if let PeerStateMachine::Stable(peer) = peer {
            peer
        } else {
            unreachable!("Peer should be in Stable state.");
        };

        let mut track_patches_after: Vec<_> = peer
            .context
            .pending_track_updates
            .iter()
            .filter_map(|t| {
                if let TrackChange::TrackPatch(patch) = t {
                    Some(patch.clone())
                } else {
                    None
                }
            })
            .collect();

        let second_track_patch = track_patches_after.pop().unwrap();
        assert_eq!(second_track_patch.is_muted, Some(false));

        let first_track_patch = track_patches_after.pop().unwrap();
        assert_eq!(first_track_patch.is_muted, Some(false));

        assert!(track_patches_after.is_empty());
    }
}
