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
//!    variant into [`PeerChange`].
//! 2. Implement your changing logic in the [`PeerChangeHandler`]
//!    implementation.
//! 3. Create a function in the [`PeerChangesScheduler`] which will schedule
//!    your change by adding it into the `peer_changes_queue` of the
//!    [`Context`].
//!
//! # Applying changes regardless of [`Peer`] state
//!
//! Sometimes you may want to apply changes immediately, and perform
//! renegotiation later. In this case you should call
//! [`PeerStateMachine::force_commit_scheduled_changes`]`.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

use std::{
    collections::{HashMap, HashSet},
    convert::{TryFrom, TryInto as _},
    fmt,
    rc::Rc,
};

use derive_more::Display;
use failure::Fail;
use medea_client_api_proto::{
    state, AudioSettings, Direction, IceCandidate, MediaSourceKind, MediaType,
    MemberId, NegotiationRole, PeerId as Id, PeerId, PeerUpdate, Track,
    TrackId, TrackPatchCommand, TrackPatchEvent, VideoSettings,
};
use medea_macro::{dispatchable, enum_delegate};

use crate::{
    api::control::endpoints::webrtc_publish_endpoint::PublishPolicy,
    media::MediaTrack,
    signalling::{
        elements::endpoints::{
            webrtc::WebRtcPublishEndpoint, Endpoint, WeakEndpoint,
        },
        peers::Counter,
    },
    turn::{IceUser, IceUsers},
};

/// Subscriber to the events indicating that [`Peer`] was updated.
#[cfg_attr(test, mockall::automock)]
pub trait PeerUpdatesSubscriber: fmt::Debug {
    /// Notifies subscriber that provided [`Peer`] must be negotiated.
    fn negotiation_needed(&self, peer_id: PeerId);

    /// Notifies subscriber that provided [`PeerUpdate`] were forcibly (without
    /// negotiation) applied to [`Peer`].
    fn force_update(&self, peer_id: PeerId, changes: Vec<PeerUpdate>);
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
    #[inline]
    #[must_use]
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
/// |    Stable     |
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
#[enum_delegate(pub fn member_id(&self) -> &MemberId)]
#[enum_delegate(pub fn partner_peer_id(&self) -> Id)]
#[enum_delegate(pub fn partner_member_id(&self) -> &MemberId)]
#[enum_delegate(pub fn local_sdp(&self) -> Option<&str>)]
#[enum_delegate(pub fn remote_sdp(&self) -> Option<&str>)]
#[enum_delegate(pub fn is_force_relayed(&self) -> bool)]
#[enum_delegate(pub fn ice_users(&self) -> &IceUsers)]
#[enum_delegate(pub fn add_ice_users(&mut self, ice_users: Vec<IceUser>))]
#[enum_delegate(pub fn endpoints(&self) -> Vec<WeakEndpoint>)]
#[enum_delegate(pub fn add_endpoint(&mut self, endpoint: &Endpoint))]
#[enum_delegate(
    pub fn receivers(&self) -> &HashMap<TrackId, Rc<MediaTrack>>
)]
#[enum_delegate(pub fn senders(&self) -> &HashMap<TrackId, Rc<MediaTrack>>)]
#[enum_delegate(
    pub fn get_updates(&self) -> Vec<PeerUpdate>
)]
#[enum_delegate(pub fn as_changes_scheduler(&mut self) -> PeerChangesScheduler)]
#[enum_delegate(fn inner_force_commit_scheduled_changes(&mut self))]
#[enum_delegate(
    pub fn add_ice_candidate(&mut self, ice_candidate: IceCandidate)
)]
#[enum_delegate(pub fn is_empty(&self) -> bool)]
#[enum_delegate(pub fn ice_candidates(&self) -> &HashSet<IceCandidate>)]
#[enum_delegate(pub fn is_ice_restart(&self) -> bool)]
#[enum_delegate(pub fn negotiation_role(&self) -> Option<NegotiationRole>)]
#[enum_delegate(pub fn is_known_to_remote(&self) -> bool)]
#[enum_delegate(pub fn force_commit_partner_changes(&mut self))]
#[enum_delegate(pub fn set_initialized(&mut self))]
#[derive(Debug)]
pub enum PeerStateMachine {
    WaitLocalSdp(Peer<WaitLocalSdp>),
    WaitRemoteSdp(Peer<WaitRemoteSdp>),
    Stable(Peer<Stable>),
}

impl PeerStateMachine {
    /// Returns all [`state::Sender`]s of this [`PeerStateMachine`].
    #[must_use]
    fn get_senders_states(&self) -> HashMap<TrackId, state::Sender> {
        self.senders()
            .iter()
            .map(|(id, sender)| {
                (
                    *id,
                    state::Sender {
                        id: sender.id(),
                        mid: sender.mid(),
                        media_type: sender.media_type().clone(),
                        receivers: vec![self.partner_member_id().clone()],
                        enabled_individual: sender
                            .send_media_state()
                            .is_enabled(),
                        enabled_general: sender.is_enabled_general(),
                        muted: sender.send_media_state().is_muted(),
                    },
                )
            })
            .collect()
    }

    /// Returns all [`state::Receiver`]s of this [`PeerStateMachine`].
    #[must_use]
    fn get_receivers_states(&self) -> HashMap<TrackId, state::Receiver> {
        self.receivers()
            .iter()
            .map(|(id, receiver)| {
                (
                    *id,
                    state::Receiver {
                        id: *id,
                        mid: receiver.mid(),
                        media_type: receiver.media_type().clone(),
                        sender_id: self.partner_member_id().clone(),
                        enabled_individual: receiver
                            .recv_media_state()
                            .is_enabled(),
                        enabled_general: receiver.is_enabled_general(),
                        muted: receiver.recv_media_state().is_muted(),
                    },
                )
            })
            .collect()
    }

    /// Returns a [`state::Peer`] of this [`PeerStateMachine`].
    ///
    /// # Panics
    ///
    /// If this [`Peer`] doesn't have an [`IceUser`].
    #[must_use]
    pub fn get_state(&self) -> state::Peer {
        state::Peer {
            id: self.id(),
            senders: self.get_senders_states(),
            receivers: self.get_receivers_states(),
            force_relay: self.is_force_relayed(),
            ice_servers: self.ice_users().try_into().unwrap(),
            negotiation_role: self.negotiation_role(),
            local_sdp: self.local_sdp().map(ToOwned::to_owned),
            remote_sdp: self.remote_sdp().map(ToOwned::to_owned),
            ice_candidates: self.ice_candidates().clone(),
            restart_ice: self.is_ice_restart(),
        }
    }

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

    /// Indicates whether this [`PeerStateMachine`] is currently in a [`Stable`]
    /// state.
    #[inline]
    #[must_use]
    pub fn is_stable(&self) -> bool {
        matches!(self, PeerStateMachine::Stable(_))
    }

    /// Indicates whether this [`PeerStateMachine`] can forcibly commit a
    /// [`PeerChange::PartnerTrackPatch`].
    #[inline]
    #[must_use]
    pub fn can_forcibly_commit_partner_patches(&self) -> bool {
        match &self {
            PeerStateMachine::Stable(peer) => peer.context.is_known_to_remote,
            PeerStateMachine::WaitLocalSdp(peer) => {
                !peer.context.is_known_to_remote
            }
            PeerStateMachine::WaitRemoteSdp(peer) => {
                peer.context.local_sdp.is_some()
                    && !peer.context.is_known_to_remote
            }
        }
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

/// Action which can be done on a negotiation process finish.
#[derive(Clone, Copy, Debug)]
enum OnNegotiationFinish {
    /// New negotiation should be started.
    Renegotiate,

    /// Nothing should be done.
    Noop,
}

/// State of a [`Peer`] initialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InitializationState {
    /// [`Peer`] is initialized.
    Done,

    /// [`Peer`] is initializing currently.
    InProgress,
}

#[derive(Debug)]
pub struct Context {
    /// [`PeerId`] of this [`Peer`].
    id: Id,

    /// [`MemberId`] of a [`Member`] which owns this [`Peer`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    member_id: MemberId,

    /// [`PeerId`] of a partner [`Peer`].
    partner_peer: Id,

    /// [`MemberId`] of a partner [`Peer`]'s owner.
    partner_member: MemberId,

    /// [`IceUser`]s created for this [`Peer`].
    ice_users: IceUsers,

    /// [SDP] offer of this [`Peer`].
    ///
    /// [SDP]: https://tools.ietf.org/html/rfc4317
    local_sdp: Option<String>,

    /// [SDP] of the partner [`Peer`].
    ///
    /// [SDP]: https://tools.ietf.org/html/rfc4317
    remote_sdp: Option<String>,

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

    /// [`Peer`] changes, that remote [`Peer`] is not aware of.
    pending_peer_changes: Vec<PeerChange>,

    /// Queue of the [`PeerChange`]s that are scheduled to apply when this
    /// [`Peer`] will be in a [`Stable`] state.
    peer_changes_queue: Vec<PeerChange>,

    /// Subscriber to the events which indicates that negotiation process
    /// should be started for this [`Peer`].
    peer_updates_sub: Rc<dyn PeerUpdatesSubscriber>,

    /// All [`IceCandidate`]s received for this [`Peer`].
    ice_candidates: HashSet<IceCandidate>,

    /// Indicator whether an ICE restart should be performed for this [`Peer`].
    ice_restart: bool,

    /// Current [`NegotiationRole`] of this [`Peer`].
    negotiation_role: Option<NegotiationRole>,

    /// Action which should be done on a negotiation process finish.
    on_negotiation_finish: OnNegotiationFinish,

    /// State of the [`Peer`] initialization.
    initialization_state: InitializationState,
}

/// [`Peer`] changes, that remote [`Peer`] is not aware of.
#[dispatchable]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeerChange {
    /// [`MediaTrack`]s with [`Direction::Send`] of this [`Peer`] that remote
    /// Peer is not aware of.
    AddSendTrack(Rc<MediaTrack>),

    /// [`MediaTrack`]s with [`Direction::Recv`] of this [`Peer`] that remote
    /// Peer is not aware of.
    AddRecvTrack(Rc<MediaTrack>),

    /// [`TrackId`] of the removed [`MediaTrack`]. Remote [`Peer`] is not aware
    /// of this change.
    RemoveTrack(TrackId),

    /// Changes to some [`MediaTrack`], that remote Peer is not aware of.
    TrackPatch(TrackPatchEvent),

    /// Changes to some [`MediaTrack`] made by this [`Peer`]s partner [`Peer`],
    /// that remote [`Peer`] is not aware of.
    PartnerTrackPatch(TrackPatchEvent),

    /// ICE restart request.
    IceRestart,
}

impl PeerChange {
    /// Indicates whether this [`PeerChange`] doesn't require renegotiation.
    #[inline]
    #[must_use]
    fn is_negotiation_state_agnostic(&self) -> bool {
        matches!(
            self,
            Self::TrackPatch(TrackPatchEvent {
                id: _,
                enabled_individual: None,
                enabled_general: None,
                muted: Some(_),
            })
        )
    }

    /// Tries to return new [`Track`] based on this [`PeerChange`].
    ///
    /// Returns `None` if this [`PeerChange`] doesn't indicates new [`Track`]
    /// creation.
    fn as_new_track(&self, partner_member_id: MemberId) -> Option<Track> {
        match self.as_peer_update(partner_member_id) {
            PeerUpdate::Added(track) => Some(track),
            PeerUpdate::Updated(_)
            | PeerUpdate::IceRestart
            | PeerUpdate::Removed(_) => None,
        }
    }

    /// Returns [`PeerUpdate`] based on this [`PeerChange`].
    fn as_peer_update(&self, partner_member_id: MemberId) -> PeerUpdate {
        match self {
            Self::AddSendTrack(track) => PeerUpdate::Added(Track {
                id: track.id(),
                media_type: track.media_type().clone(),
                direction: Direction::Send {
                    receivers: vec![partner_member_id],
                    mid: track.mid(),
                },
            }),
            Self::AddRecvTrack(track) => PeerUpdate::Added(Track {
                id: track.id(),
                media_type: track.media_type().clone(),
                direction: Direction::Recv {
                    sender: partner_member_id,
                    mid: track.mid(),
                },
            }),
            Self::RemoveTrack(track_id) => PeerUpdate::Removed(*track_id),
            Self::TrackPatch(track_patch)
            | Self::PartnerTrackPatch(track_patch) => {
                PeerUpdate::Updated(track_patch.clone())
            }
            Self::IceRestart => PeerUpdate::IceRestart,
        }
    }

    /// Returns `true` if this [`PeerChange`] can be forcibly applied.
    fn can_force_apply(&self) -> bool {
        match self {
            Self::AddSendTrack(_)
            | Self::AddRecvTrack(_)
            | Self::IceRestart
            | Self::RemoveTrack(_) => false,
            Self::TrackPatch(_) | Self::PartnerTrackPatch(_) => true,
        }
    }
}

impl<T> PeerChangeHandler for Peer<T> {
    type Output = PeerChange;

    /// Inserts the provided [`MediaTrack`] into `senders` of this [`Context`].
    #[inline]
    fn on_add_send_track(&mut self, track: Rc<MediaTrack>) -> Self::Output {
        self.context.senders.insert(track.id(), Rc::clone(&track));

        PeerChange::AddSendTrack(track)
    }

    /// Inserts the provided [`MediaTrack`] into `receivers` of this
    /// [`Context`].
    #[inline]
    fn on_add_recv_track(&mut self, track: Rc<MediaTrack>) -> Self::Output {
        self.context.receivers.insert(track.id(), Rc::clone(&track));

        PeerChange::AddRecvTrack(track)
    }

    /// Applies provided [`TrackPatchEvent`] to [`Peer`]s [`Track`].
    fn on_track_patch(&mut self, mut patch: TrackPatchEvent) -> Self::Output {
        let (track, is_tx) = if let Some(tx) = self.senders().get(&patch.id) {
            (tx, true)
        } else if let Some(rx) = self.receivers().get(&patch.id) {
            (rx, false)
        } else {
            return PeerChange::TrackPatch(patch);
        };

        if let Some(enabled) = patch.enabled_individual {
            if is_tx {
                track.send_media_state().set_enabled(enabled);
            } else {
                track.recv_media_state().set_enabled(enabled);
            }
            patch.enabled_general = Some(track.is_enabled_general());
        }
        if let Some(muted) = patch.muted {
            if is_tx {
                track.send_media_state().set_muted(muted);
            } else {
                track.recv_media_state().set_muted(muted);
            }
        }

        PeerChange::TrackPatch(patch)
    }

    /// Applies provided [`TrackPatchEvent`] that is sourced from this [`Peer`]s
    /// partner [`Peer`] to some shared [`Track`].
    fn on_partner_track_patch(
        &mut self,
        mut patch: TrackPatchEvent,
    ) -> Self::Output {
        if let Some(enabled_individual) = patch.enabled_individual {
            // Resets `enabled_individual` to `None`. Sets `enabled_general` to
            // `Some` if provided `enabled_individual` is equal to the real
            // general media exchange state.
            patch.enabled_individual = None;
            let track = self
                .senders()
                .get(&patch.id)
                .or_else(|| self.receivers().get(&patch.id));

            if let Some(track) = track {
                if enabled_individual == track.is_enabled_general() {
                    patch.enabled_general = Some(track.is_enabled_general());
                }
            }
        }

        PeerChange::TrackPatch(patch)
    }

    /// Removes [`MediaTrack`] with the provided [`TrackId`] from this [`Peer`].
    #[inline]
    fn on_remove_track(&mut self, track_id: TrackId) -> Self::Output {
        self.context.senders.remove(&track_id);
        self.context.receivers.remove(&track_id);
        PeerChange::RemoveTrack(track_id)
    }

    /// Sets the `ice_restart` flag of this [`Context`] to `true`.
    #[inline]
    fn on_ice_restart(&mut self) -> Self::Output {
        self.context.ice_restart = true;
        PeerChange::IceRestart
    }
}

/// Deduper of the [`TrackPatchEvent`]s.
///
/// Responsible for merging [`TrackPatchEvent`]s from different sources (queue,
/// pending updates).
struct TrackPatchDeduper {
    /// All merged [`TrackPatchEvent`]s from this [`TrackPatchDeduper`].
    result: HashMap<TrackId, TrackPatchEvent>,

    /// [`TrackId`]s that can be merged.
    ///
    /// If [`None`] then all [`TrackPatchEvent`]s can be merged.
    whitelist: Option<HashSet<TrackId>>,
}

impl TrackPatchDeduper {
    /// Returns new [`TrackPatchDeduper`].
    fn new() -> Self {
        Self {
            result: HashMap::new(),
            whitelist: None,
        }
    }

    /// Returns new [`TrackPatchDeduper`] with the provided whitelist, meaning
    /// that [`TrackPatchDeduper::drain_merge`] will drain only [`PeerChange`]s
    /// with [`TrackId`]s in the provided set.
    fn with_whitelist(whitelist: HashSet<TrackId>) -> Self {
        Self {
            result: HashMap::new(),
            whitelist: Some(whitelist),
        }
    }

    /// Drains mergeable [`TrackPatchEvent`]s from the provided [`Vec`], merging
    /// those to accumulative [`TrackPatchEvent`]s list inside this struct.
    fn drain_merge(&mut self, changes: &mut Vec<PeerChange>) {
        changes.retain(|change| {
            if !change.can_force_apply() {
                return true;
            }
            let patch = match change {
                PeerChange::TrackPatch(patch)
                | PeerChange::PartnerTrackPatch(patch) => patch,
                _ => return true,
            };

            if self.whitelist.is_some()
                && !self.whitelist.as_ref().unwrap().contains(&patch.id)
            {
                return true;
            }

            self.result
                .entry(patch.id)
                .or_insert_with(|| TrackPatchEvent::new(patch.id))
                .merge(patch);
            false
        });
    }

    /// Returns [`Iterator`] with all previously merged [`PeerChange`]s.
    fn into_inner(self) -> impl Iterator<Item = PeerChange> {
        self.result
            .into_iter()
            .map(|(_, patch)| PeerChange::TrackPatch(patch))
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
    /// [`Member`]: crate::signalling::elements::Member
    #[inline]
    pub fn member_id(&self) -> &MemberId {
        &self.context.member_id
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
    /// [`Member`]: crate::signalling::elements::Member
    #[inline]
    pub fn partner_member_id(&self) -> &MemberId {
        &self.context.partner_member
    }

    /// Returns SDP offer of this [`Peer`].
    #[inline]
    #[must_use]
    pub fn local_sdp(&self) -> Option<&str> {
        self.context.local_sdp.as_deref()
    }

    /// Returns SDP offer of the partner [`Peer`].
    #[inline]
    #[must_use]
    pub fn remote_sdp(&self) -> Option<&str> {
        self.context.remote_sdp.as_deref()
    }

    /// Returns [`PeerUpdate`]s of this [`Peer`] which should be sent to the
    /// client in the [`Event::PeerUpdated`].
    ///
    /// [`Event::PeerUpdated`]: medea_client_api_proto::Event::PeerUpdated
    pub fn get_updates(&self) -> Vec<PeerUpdate> {
        self.context
            .pending_peer_changes
            .iter()
            .map(|c| c.as_peer_update(self.partner_member_id().clone()))
            .collect()
    }

    /// Returns [`Track`]s that remote [`Peer`] is not aware of.
    pub fn new_tracks(&self) -> Vec<Track> {
        self.context
            .pending_peer_changes
            .iter()
            .filter_map(|c| c.as_new_track(self.partner_member_id().clone()))
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

    /// Returns [`IceUsers`] created for this [`Peer`].
    #[inline]
    pub fn ice_users(&self) -> &IceUsers {
        &self.context.ice_users
    }

    /// Adds [`IceUser`]s, which is used to generate [`IceServer`]s of this
    /// [`Peer`].
    ///
    /// [`IceServer`]: medea_client_api_proto::IceServer
    #[inline]
    pub fn add_ice_users(&mut self, ice_users: Vec<IceUser>) {
        self.context.ice_users.add(ice_users);
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
                play.set_peer_id_and_partner_peer_id(
                    self.id(),
                    self.partner_peer_id(),
                );
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

    /// Forcibly commits all the [`PeerChange::PartnerTrackPatch`]es.
    pub fn force_commit_partner_changes(&mut self) {
        let mut partner_patches = Vec::new();
        // TODO: use drain_filter when its stable
        let mut i = 0;
        while i != self.context.peer_changes_queue.len() {
            if matches!(
                self.context.peer_changes_queue[i],
                PeerChange::PartnerTrackPatch(_)
            ) {
                let change = self
                    .context
                    .peer_changes_queue
                    .remove(i)
                    .dispatch_with(self);
                partner_patches.push(change);
            } else {
                i += 1;
            }
        }

        let mut deduper = TrackPatchDeduper::with_whitelist(
            partner_patches
                .iter()
                .filter_map(|t| match t {
                    PeerChange::PartnerTrackPatch(patch) => Some(patch.id),
                    _ => None,
                })
                .collect(),
        );
        deduper.drain_merge(&mut self.context.pending_peer_changes);
        deduper.drain_merge(&mut partner_patches);

        let updates: Vec<_> = deduper
            .into_inner()
            .map(|c| c.as_peer_update(self.partner_member_id().clone()))
            .collect();

        if !updates.is_empty() {
            self.context
                .peer_updates_sub
                .force_update(self.id(), updates);
        }
    }

    /// Commits all [`PeerChange`]s which are marked as forcible.
    pub fn inner_force_commit_scheduled_changes(&mut self) {
        let mut forcible_changes = Vec::new();
        let mut filtered_changes_queue = Vec::new();
        // TODO: use drain_filter when its stable
        for change in std::mem::take(&mut self.context.peer_changes_queue) {
            if change.can_force_apply() {
                forcible_changes.push(change.dispatch_with(self));
            } else {
                filtered_changes_queue.push(change);
            }
        }
        self.context.peer_changes_queue = filtered_changes_queue;

        let mut deduper = TrackPatchDeduper::with_whitelist(
            forcible_changes
                .iter()
                .filter_map(|t| match t {
                    PeerChange::TrackPatch(patch)
                    | PeerChange::PartnerTrackPatch(patch) => Some(patch.id),
                    _ => None,
                })
                .collect(),
        );
        deduper.drain_merge(&mut self.context.pending_peer_changes);
        deduper.drain_merge(&mut forcible_changes);

        let updates: Vec<_> = deduper
            .into_inner()
            .map(|c| c.as_peer_update(self.partner_member_id().clone()))
            .collect();

        if !updates.is_empty() {
            // TODO: Renegotiation can be skipped if we will check that
            //       client doesn't sent his local offer to us atm, if so, then
            //       renegotiation for this force update is not needed, because
            //       those changes will be reflected in the local offer, when
            //       client will send it.
            if self.context.is_known_to_remote {
                self.context.on_negotiation_finish =
                    OnNegotiationFinish::Renegotiate;
            }
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

    /// Returns `true` if this [`Peer`] doesn't have any `Send` and `Recv`
    /// [`MediaTrack`]s.
    pub fn is_empty(&self) -> bool {
        if self.context.senders.is_empty() && self.context.receivers.is_empty()
        {
            return true;
        }

        let removed_tracks: HashSet<_> = self
            .context
            .peer_changes_queue
            .iter()
            .filter_map(|change| match change {
                PeerChange::RemoveTrack(track_id) => Some(*track_id),
                _ => None,
            })
            .collect();
        let peers_tracks: HashSet<_> = self
            .context
            .senders
            .iter()
            .chain(self.context.receivers.iter())
            .map(|t| *t.0)
            .collect();

        removed_tracks == peers_tracks
    }

    /// Returns [`PeerChangesScheduler`] for this [`Peer`].
    #[inline]
    #[must_use]
    pub fn as_changes_scheduler(&mut self) -> PeerChangesScheduler {
        PeerChangesScheduler {
            context: &mut self.context,
        }
    }

    /// Deduplicates pending [`PeerChange`]s.
    fn dedup_pending_track_updates(&mut self) {
        self.dedup_ice_restarts();
        self.dedup_track_patches();
    }

    /// Dedupes [`PeerChange::IceRestart`]s.
    fn dedup_ice_restarts(&mut self) {
        let pending_track_updates = &mut self.context.pending_peer_changes;
        let last_ice_restart_rev_index = pending_track_updates
            .iter()
            .rev()
            .position(|item| matches!(item, PeerChange::IceRestart));
        if let Some(idx) = last_ice_restart_rev_index {
            let last_ice_restart_index = pending_track_updates.len() - 1 - idx;
            pending_track_updates.retain({
                let mut i = 0;
                move |item| {
                    let is_last_ice_restart = i == last_ice_restart_index;
                    i += 1;
                    is_last_ice_restart
                        || !matches!(item, PeerChange::IceRestart)
                }
            });
        }
    }

    /// Dedupes [`PeerChange`]s from this [`Peer`].
    fn dedup_track_patches(&mut self) {
        let mut deduper = TrackPatchDeduper::new();
        deduper.drain_merge(&mut self.context.pending_peer_changes);
        self.context
            .pending_peer_changes
            .extend(deduper.into_inner());
    }

    /// Adds the provided [`IceCandidate`] to this [`Peer`].
    #[inline]
    pub fn add_ice_candidate(&mut self, ice_candidate: IceCandidate) {
        self.context.ice_candidates.insert(ice_candidate);
    }

    /// Returns all [`IceCandidate`]s received for this [`Peer`].
    #[inline]
    #[must_use]
    pub fn ice_candidates(&self) -> &HashSet<IceCandidate> {
        &self.context.ice_candidates
    }

    /// Indicates whether an ICE restart should be performed for this [`Peer`].
    #[inline]
    #[must_use]
    pub fn is_ice_restart(&self) -> bool {
        self.context.ice_restart
    }

    /// Returns the current [`NegotiationRole`] of this [`Peer`].
    #[inline]
    #[must_use]
    pub fn negotiation_role(&self) -> Option<NegotiationRole> {
        self.context.negotiation_role.clone()
    }

    /// Marks this [`Peer`] as initialized.
    #[inline]
    pub fn set_initialized(&mut self) {
        self.context.initialization_state = InitializationState::Done;
    }
}

impl Peer<WaitLocalSdp> {
    /// Sets local description and transition [`Peer`] to [`WaitRemoteSdp`]
    /// state.
    #[inline]
    #[must_use]
    pub fn set_local_offer(self, local_sdp: String) -> Peer<WaitRemoteSdp> {
        let mut context = self.context;
        context.local_sdp = Some(local_sdp);
        Peer {
            context,
            state: WaitRemoteSdp {},
        }
    }

    /// Sets local description and transition [`Peer`] to [`Stable`]
    /// state.
    #[inline]
    #[must_use]
    pub fn set_local_answer(self, sdp_answer: String) -> Peer<Stable> {
        let mut context = self.context;
        context.local_sdp = Some(sdp_answer);
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
                .ok_or_else(|| PeerError::MidsMismatch(track.id()))?;
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
                sender.set_transceiver_enabled(is_publishing);
            }
        }
    }
}

impl Peer<WaitRemoteSdp> {
    /// Sets remote description and transitions [`Peer`] to [`Stable`] state.
    #[inline]
    #[must_use]
    pub fn set_remote_answer(mut self, sdp_answer: String) -> Peer<Stable> {
        self.context.remote_sdp = Some(sdp_answer);

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
    #[must_use]
    pub fn set_remote_offer(mut self, sdp_offer: String) -> Peer<WaitLocalSdp> {
        self.context.negotiation_role =
            Some(NegotiationRole::Answerer(sdp_offer.clone()));
        self.context.remote_sdp = Some(sdp_offer);

        Peer {
            context: self.context,
            state: WaitLocalSdp {},
        }
    }
}

impl Peer<Stable> {
    /// Creates new [`Peer`] for [`Member`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    pub fn new(
        id: Id,
        member_id: MemberId,
        partner_peer: Id,
        partner_member: MemberId,
        is_force_relayed: bool,
        peer_updates_sub: Rc<dyn PeerUpdatesSubscriber>,
    ) -> Self {
        let context = Context {
            id,
            member_id,
            partner_peer,
            partner_member,
            ice_users: IceUsers::new(),
            local_sdp: None,
            remote_sdp: None,
            receivers: HashMap::new(),
            senders: HashMap::new(),
            is_force_relayed,
            endpoints: Vec::new(),
            is_known_to_remote: false,
            pending_peer_changes: Vec::new(),
            peer_changes_queue: Vec::new(),
            peer_updates_sub,
            ice_candidates: HashSet::new(),
            ice_restart: false,
            negotiation_role: None,
            on_negotiation_finish: OnNegotiationFinish::Noop,
            initialization_state: InitializationState::InProgress,
        };

        Self {
            context,
            state: Stable {},
        }
    }

    /// Changes [`Peer`] state to [`WaitLocalSdp`] and discards previously saved
    /// [SDP] Offer and Answer.
    ///
    /// Resets the `local_sdp` and the `remote_sdp` of this [`Context`].
    ///
    /// [SDP]: https://tools.ietf.org/html/rfc4317
    #[inline]
    #[must_use]
    pub fn start_as_offerer(self) -> Peer<WaitLocalSdp> {
        let mut context = self.context;
        context.local_sdp = None;
        context.remote_sdp = None;

        context.negotiation_role = Some(NegotiationRole::Offerer);

        Peer {
            context,
            state: WaitLocalSdp {},
        }
    }

    /// Changes [`Peer`] state to [`WaitLocalSdp`] and discards previously saved
    /// [SDP] Offer and Answer.
    ///
    /// Resets the `local_sdp` and the `remote_sdp` of this [`Context`].
    ///
    /// [SDP]: https://tools.ietf.org/html/rfc4317
    #[inline]
    #[must_use]
    pub fn start_as_answerer(self) -> Peer<WaitRemoteSdp> {
        let mut context = self.context;
        context.local_sdp = None;
        context.remote_sdp = None;

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
                    .ok_or_else(|| PeerError::MidsMismatch(track.id()))?,
            );
        }
        Ok(mids)
    }

    /// Applies previously scheduled [`PeerChange`]s to this [`Peer`], marks
    /// those changes as applied, so they can be retrieved via
    /// [`PeerStateMachine::get_updates`]. Calls
    /// [`PeerUpdatesSubscriber::negotiation_needed`] notifying subscriber that
    /// this [`Peer`] has changes to negotiate. Changes that can be applied
    /// regardless of negotiation state will be immediately force-pushed to
    /// [`PeerUpdatesSubscriber`].
    fn commit_scheduled_changes(&mut self) {
        // If InitializationState is not Done, then we can safely skip this
        // negotiation, because these changes will be committed by ongoing
        // initializer.
        if self.context.initialization_state == InitializationState::Done
            && (!self.context.peer_changes_queue.is_empty()
                || matches!(
                    self.context.on_negotiation_finish,
                    OnNegotiationFinish::Renegotiate
                ))
        {
            let mut negotiationless_changes = Vec::new();
            for task in std::mem::take(&mut self.context.peer_changes_queue) {
                let change = task.dispatch_with(self);
                if change.is_negotiation_state_agnostic() {
                    negotiationless_changes.push(change);
                } else {
                    self.context.pending_peer_changes.push(change);
                }
            }

            self.dedup_pending_track_updates();

            if self.context.pending_peer_changes.is_empty()
                && matches!(
                    self.context.on_negotiation_finish,
                    OnNegotiationFinish::Noop
                )
            {
                self.context.peer_updates_sub.force_update(
                    self.id(),
                    negotiationless_changes
                        .into_iter()
                        .map(|c| {
                            c.as_peer_update(self.partner_member_id().clone())
                        })
                        .collect(),
                );
            } else {
                self.context
                    .pending_peer_changes
                    .append(&mut negotiationless_changes);
                self.context.peer_updates_sub.negotiation_needed(self.id());
                self.context.on_negotiation_finish = OnNegotiationFinish::Noop;
            }
        }
    }

    /// Sets [`Context::is_known_to_remote`] to `true`.
    ///
    /// Sets [`Context::ice_restart`] to `false`.
    ///
    /// Resets [`Context::pending_track_updates`] buffer.
    ///
    /// Applies all scheduled changes.
    ///
    /// Should be called when negotiation was finished.
    fn negotiation_finished(&mut self) {
        self.context.ice_restart = false;
        self.context.is_known_to_remote = true;
        self.context.pending_peer_changes.clear();
        self.context.negotiation_role = None;
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
    /// Schedules provided [`TrackPatchCommand`]s as
    /// [`PeerChange::TrackPatch`].
    pub fn patch_tracks(&mut self, patches: Vec<TrackPatchCommand>) {
        for patch in patches {
            self.schedule_change(PeerChange::TrackPatch(patch.into()));
        }
    }

    /// Schedules provided [`TrackPatchCommand`] as
    /// [`PeerChange::PartnerTrackPatch`].
    pub fn partner_patch_tracks(&mut self, patches: Vec<TrackPatchCommand>) {
        for patch in patches {
            self.schedule_change(PeerChange::PartnerTrackPatch(patch.into()));
        }
    }

    /// Schedules [`PeerChange::IceRestart`].
    #[inline]
    pub fn restart_ice(&mut self) {
        self.schedule_change(PeerChange::IceRestart);
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
                    required: audio_settings.publish_policy.required(),
                }),
            ));
            self.add_sender(Rc::clone(&track_audio));
            src.add_track_id(self.context.id, track_audio.id());
            partner_peer
                .as_changes_scheduler()
                .add_receiver(track_audio);
        }

        let video_settings = src.video_settings();
        if video_settings.publish_policy != PublishPolicy::Disabled {
            let camera_video_track = Rc::new(MediaTrack::new(
                tracks_counter.next_id(),
                MediaType::Video(VideoSettings {
                    required: video_settings.publish_policy.required(),
                    source_kind: MediaSourceKind::Device,
                }),
            ));
            self.add_sender(Rc::clone(&camera_video_track));
            src.add_track_id(self.context.id, camera_video_track.id());
            partner_peer
                .as_changes_scheduler()
                .add_receiver(camera_video_track);
            let display_video_track = Rc::new(MediaTrack::new(
                tracks_counter.next_id(),
                MediaType::Video(VideoSettings {
                    required: false,
                    source_kind: MediaSourceKind::Display,
                }),
            ));
            self.add_sender(Rc::clone(&display_video_track));
            src.add_track_id(self.context.id, display_video_track.id());
            partner_peer
                .as_changes_scheduler()
                .add_receiver(display_video_track);
        }
    }

    /// Adds provided [`PeerChange`] to scheduled changes queue.
    #[inline]
    fn schedule_change(&mut self, job: PeerChange) {
        self.context.peer_changes_queue.push(job);
    }

    /// Schedules [`Track`] addition to [`Peer`] receive tracks list.
    ///
    /// This [`Track`] will be considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks` after this scheduled
    /// [`PeerChange`] will be applied.
    #[inline]
    fn add_receiver(&mut self, track: Rc<MediaTrack>) {
        self.schedule_change(PeerChange::AddRecvTrack(track));
    }

    /// Schedules [`Track`] addition to [`Peer`] send tracks list.
    ///
    /// This [`Track`] will be considered new (not known to remote) and may be
    /// obtained by calling `Peer.new_tracks` after this scheduled
    /// [`PeerChange`] will be applied.
    #[inline]
    fn add_sender(&mut self, track: Rc<MediaTrack>) {
        self.schedule_change(PeerChange::AddSendTrack(track));
    }

    /// Removes [`Track`]s with a provided [`TrackId`]s from this [`Peer`].
    pub fn remove_tracks(&mut self, track_ids: &[TrackId]) {
        for id in track_ids {
            let changes_indexes_to_remove: Vec<_> = self
                .context
                .peer_changes_queue
                .iter()
                .enumerate()
                .filter_map(|(n, change)| match change {
                    PeerChange::AddSendTrack(track)
                    | PeerChange::AddRecvTrack(track) => {
                        if track.id() == *id {
                            Some(n)
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .collect();
            if changes_indexes_to_remove.is_empty() {
                self.schedule_change(PeerChange::RemoveTrack(*id))
            } else {
                for remove_index in changes_indexes_to_remove {
                    self.context.peer_changes_queue.remove(remove_index);
                }
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Returns dummy [`PeerUpdatesSubscriber`] mock which does nothing.
    pub fn dummy_negotiation_sub_mock() -> Rc<dyn PeerUpdatesSubscriber> {
        let mut mock = MockPeerUpdatesSubscriber::new();
        mock.expect_negotiation_needed().returning(drop);

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
                MediaType::Audio(AudioSettings { required: true }),
            );
            peer.context.senders.insert(track_id, Rc::new(track));
        }

        for _ in 0..send_video {
            let track_id = track_id_counter.next_id();
            let track = MediaTrack::new(
                track_id,
                MediaType::Video(VideoSettings {
                    required: true,
                    source_kind: MediaSourceKind::Device,
                }),
            );
            peer.context.senders.insert(track_id, Rc::new(track));
        }

        for _ in 0..recv_audio {
            let track_id = track_id_counter.next_id();
            let track = MediaTrack::new(
                track_id,
                MediaType::Audio(AudioSettings { required: true }),
            );
            peer.context.receivers.insert(track_id, Rc::new(track));
        }

        for _ in 0..recv_video {
            let track_id = track_id_counter.next_id();
            let track = MediaTrack::new(
                track_id,
                MediaType::Video(VideoSettings {
                    required: true,
                    source_kind: MediaSourceKind::Device,
                }),
            );
            peer.context.receivers.insert(track_id, Rc::new(track));
        }

        peer.into()
    }

    fn media_track(track_id: u32) -> Rc<MediaTrack> {
        Rc::new(MediaTrack::new(
            TrackId(track_id),
            MediaType::Video(VideoSettings {
                required: true,
                source_kind: MediaSourceKind::Device,
            }),
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
            MemberId::from("member-1"),
            PeerId(1),
            MemberId::from("member-2"),
            false,
            Rc::new(negotiation_sub),
        );
        peer.add_ice_users(vec![IceUser::new_coturn_static(
            String::new(),
            String::new(),
            String::new(),
        )]);
        peer.set_initialized();

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

        let mut peer = Peer::new(
            PeerId(0),
            MemberId::from("member-1"),
            PeerId(1),
            MemberId::from("member-2"),
            false,
            Rc::new(negotiation_sub),
        );
        peer.add_ice_users(vec![IceUser::new_coturn_static(
            String::new(),
            String::new(),
            String::new(),
        )]);
        peer.set_initialized();

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
        assert_eq!(peer.context.pending_peer_changes.len(), 2);
        assert_eq!(peer.context.peer_changes_queue.len(), 0);
        assert_eq!(rx.recv().unwrap(), PeerId(0));
    }

    #[test]
    fn force_updates_works() {
        let (force_update_tx, force_update_rx) = std::sync::mpsc::channel();
        let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
        negotiation_sub.expect_force_update().returning(
            move |peer_id: PeerId, changes: Vec<PeerUpdate>| {
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
            MemberId::from("member-1"),
            PeerId(1),
            MemberId::from("member-2"),
            false,
            Rc::new(negotiation_sub),
        );
        peer.add_ice_users(vec![IceUser::new_coturn_static(
            String::new(),
            String::new(),
            String::new(),
        )]);
        peer.set_initialized();
        peer.context.is_known_to_remote = true;
        peer.as_changes_scheduler().add_sender(media_track(0));
        peer.as_changes_scheduler().add_receiver(media_track(1));
        peer.commit_scheduled_changes();

        let peer_id = negotiation_needed_rx.recv().unwrap();
        assert_eq!(peer_id, PeerId(0));

        let mut peer = peer.start_as_offerer();

        peer.as_changes_scheduler().patch_tracks(vec![
            TrackPatchCommand {
                id: TrackId(0),
                enabled: Some(false),
                muted: None,
            },
            TrackPatchCommand {
                id: TrackId(1),
                enabled: Some(false),
                muted: None,
            },
        ]);
        peer.inner_force_commit_scheduled_changes();
        let (peer_id, changes) = force_update_rx.recv().unwrap();

        assert_eq!(peer_id, PeerId(0));
        assert_eq!(changes.len(), 2);
        assert!(peer.context.peer_changes_queue.is_empty());
        assert!(matches!(
            peer.context.on_negotiation_finish,
            OnNegotiationFinish::Renegotiate
        ));

        let peer = peer.set_local_offer(String::new());
        let _ = peer.set_remote_answer(String::new());

        let peer_id = negotiation_needed_rx.recv().unwrap();
        assert_eq!(peer_id, PeerId(0));
    }

    #[test]
    fn track_patch_dedup_works() {
        let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
        negotiation_sub
            .expect_force_update()
            .returning(move |_: PeerId, _: Vec<PeerUpdate>| {});
        negotiation_sub
            .expect_negotiation_needed()
            .returning(move |_: PeerId| {});
        let mut peer = Peer::new(
            PeerId(0),
            MemberId::from("member-1"),
            PeerId(1),
            MemberId::from("member-2"),
            false,
            Rc::new(negotiation_sub),
        );

        let patches = vec![
            TrackPatchCommand {
                id: TrackId(1),
                enabled: Some(false),
                muted: None,
            },
            TrackPatchCommand {
                id: TrackId(2),
                enabled: None,
                muted: None,
            },
            TrackPatchCommand {
                id: TrackId(1),
                enabled: Some(true),
                muted: None,
            },
            TrackPatchCommand {
                id: TrackId(2),
                enabled: Some(false),
                muted: None,
            },
            TrackPatchCommand {
                id: TrackId(2),
                enabled: Some(true),
                muted: None,
            },
            TrackPatchCommand {
                id: TrackId(2),
                enabled: None,
                muted: None,
            },
            TrackPatchCommand {
                id: TrackId(1),
                enabled: None,
                muted: None,
            },
        ];
        peer.as_changes_scheduler().patch_tracks(patches);
        let mut peer = PeerStateMachine::from(peer);
        peer.add_ice_users(vec![IceUser::new_coturn_static(
            String::new(),
            String::new(),
            String::new(),
        )]);
        peer.set_initialized();
        peer.commit_scheduled_changes();
        let peer = if let PeerStateMachine::Stable(peer) = peer {
            peer
        } else {
            unreachable!("Peer should be in Stable state.");
        };

        let mut track_patches_after: Vec<_> = peer
            .context
            .pending_peer_changes
            .iter()
            .filter_map(|t| {
                if let PeerChange::TrackPatch(patch) = t {
                    Some(patch.clone())
                } else {
                    None
                }
            })
            .collect();

        let second_track_patch = track_patches_after.pop().unwrap();
        assert_eq!(second_track_patch.enabled_individual, Some(true));

        let first_track_patch = track_patches_after.pop().unwrap();
        assert_eq!(first_track_patch.enabled_general, None);

        assert!(track_patches_after.is_empty());
    }

    /// Checks that [`PeerChange::IceRestart`] correctly dedups.
    #[test]
    fn ice_restart_dedupping_works() {
        let changes = vec![
            PeerChange::IceRestart,
            PeerChange::IceRestart,
            PeerChange::IceRestart,
            PeerChange::TrackPatch(TrackPatchEvent {
                id: TrackId(0),
                enabled_individual: None,
                enabled_general: None,
                muted: None,
            }),
            PeerChange::IceRestart,
            PeerChange::TrackPatch(TrackPatchEvent {
                id: TrackId(0),
                enabled_individual: None,
                enabled_general: None,
                muted: None,
            }),
        ];

        let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
        negotiation_sub
            .expect_force_update()
            .returning(move |_: PeerId, _: Vec<PeerUpdate>| {});
        negotiation_sub
            .expect_negotiation_needed()
            .returning(move |_: PeerId| {});
        let mut peer = Peer::new(
            PeerId(0),
            MemberId::from("member-1"),
            PeerId(1),
            MemberId::from("member-2"),
            false,
            Rc::new(negotiation_sub),
        );

        peer.context.pending_peer_changes = changes;

        peer.dedup_ice_restarts();

        let deduped_track_updates = peer.context.pending_peer_changes;
        assert_eq!(deduped_track_updates.len(), 3);
        assert!(matches!(deduped_track_updates[1], PeerChange::IceRestart));
    }

    /// Checks that [`Peer::inner_force_commit_scheduled_changes`] merges
    /// changes from the [`Context::pending_track_updates`] with a forcible
    /// changes from the [`Context::track_changes_queue`].
    #[test]
    fn force_update_dedups_normally() {
        let mut peer_updates_sub = MockPeerUpdatesSubscriber::new();
        peer_updates_sub.expect_force_update().times(1).returning(
            |peer_id, changes| {
                assert_eq!(peer_id, PeerId(0));
                assert_eq!(changes.len(), 1);
                if let PeerUpdate::Updated(patch) = &changes[0] {
                    assert_eq!(patch.id, TrackId(0));
                    assert_eq!(patch.enabled_individual, Some(false));
                } else {
                    unreachable!();
                }
            },
        );

        let mut peer = Peer::new(
            PeerId(0),
            MemberId::from("alice"),
            PeerId(1),
            MemberId::from("bob"),
            false,
            Rc::new(peer_updates_sub),
        );
        peer.context.pending_peer_changes = vec![
            PeerChange::TrackPatch(TrackPatchEvent {
                id: TrackId(0),
                enabled_general: Some(false),
                enabled_individual: Some(false),
                muted: None,
            }),
            PeerChange::TrackPatch(TrackPatchEvent {
                id: TrackId(0),
                enabled_general: Some(true),
                enabled_individual: Some(true),
                muted: None,
            }),
            PeerChange::TrackPatch(TrackPatchEvent {
                id: TrackId(1),
                enabled_general: Some(false),
                enabled_individual: Some(false),
                muted: None,
            }),
        ];
        peer.as_changes_scheduler().patch_tracks(vec![
            TrackPatchCommand {
                id: TrackId(0),
                enabled: Some(false),
                muted: None,
            },
            TrackPatchCommand {
                id: TrackId(0),
                enabled: Some(true),
                muted: None,
            },
            TrackPatchCommand {
                id: TrackId(0),
                enabled: Some(false),
                muted: None,
            },
        ]);
        peer.inner_force_commit_scheduled_changes();

        assert_eq!(peer.context.peer_changes_queue.len(), 0);
        assert_eq!(peer.context.pending_peer_changes.len(), 1);
        let filtered_track_change =
            peer.context.pending_peer_changes.pop().unwrap();
        if let PeerChange::TrackPatch(patch) = filtered_track_change {
            assert_eq!(patch.id, TrackId(1));
            assert_eq!(patch.enabled_general, Some(false));
        } else {
            unreachable!();
        }
    }

    /// Tests for the [`TrackPatchDeduper`].
    mod track_patch_deduper {
        use super::*;

        /// Checks that [`TrackPatchDeduper::with_whitelist`] filters
        /// [`TrackPatchEvent`]s which are not listed in the whitelist.
        #[test]
        fn whitelisting_works() {
            let mut deduper =
                TrackPatchDeduper::with_whitelist(hashset![TrackId(1)]);
            let filtered_patch = PeerChange::TrackPatch(TrackPatchEvent {
                id: TrackId(2),
                enabled_general: Some(false),
                enabled_individual: Some(false),
                muted: None,
            });
            let whitelisted_patch = PeerChange::TrackPatch(TrackPatchEvent {
                id: TrackId(1),
                enabled_general: Some(false),
                enabled_individual: Some(false),
                muted: None,
            });
            let mut patches =
                vec![whitelisted_patch.clone(), filtered_patch.clone()];
            deduper.drain_merge(&mut patches);
            assert_eq!(patches.len(), 1);
            assert_eq!(patches[0], filtered_patch);

            let merged_changes: Vec<_> = deduper.into_inner().collect();
            assert_eq!(merged_changes.len(), 1);
            assert_eq!(merged_changes[0], whitelisted_patch);
        }

        /// Checks that [`TrackPatchDeduper`] merges [`PeerChange`]s correctly.
        #[test]
        fn merging_works() {
            let mut deduper = TrackPatchDeduper::new();

            let mut changes: Vec<_> = vec![
                TrackPatchEvent {
                    id: TrackId(1),
                    enabled_general: Some(true),
                    enabled_individual: Some(true),
                    muted: None,
                },
                TrackPatchEvent {
                    id: TrackId(2),
                    enabled_general: Some(false),
                    enabled_individual: Some(false),
                    muted: None,
                },
                TrackPatchEvent {
                    id: TrackId(1),
                    enabled_general: Some(false),
                    enabled_individual: Some(false),
                    muted: None,
                },
                TrackPatchEvent {
                    id: TrackId(1),
                    enabled_general: None,
                    enabled_individual: None,
                    muted: None,
                },
                TrackPatchEvent {
                    id: TrackId(2),
                    enabled_general: Some(true),
                    enabled_individual: Some(true),
                    muted: None,
                },
            ]
            .into_iter()
            .map(|p| PeerChange::TrackPatch(p))
            .collect();
            let unrelated_change =
                PeerChange::AddSendTrack(Rc::new(MediaTrack::new(
                    TrackId(1),
                    MediaType::Audio(AudioSettings { required: true }),
                )));
            changes.push(unrelated_change.clone());
            deduper.drain_merge(&mut changes);

            assert_eq!(changes.len(), 1);
            assert_eq!(changes[0], unrelated_change);

            let merged_changes: HashMap<_, _> = deduper
                .into_inner()
                .filter_map(|t| {
                    if let PeerChange::TrackPatch(patch) = t {
                        Some((patch.id, patch))
                    } else {
                        None
                    }
                })
                .collect();

            assert_eq!(merged_changes.len(), 2);
            {
                let track_1 = merged_changes.get(&TrackId(1)).unwrap();
                assert_eq!(track_1.enabled_general, Some(false));
            }
            {
                let track_2 = merged_changes.get(&TrackId(2)).unwrap();
                assert_eq!(track_2.enabled_general, Some(true));
            }
        }
    }

    mod negotiation_state_agnostic_patches {
        use super::*;

        /// Checks that negotiation state agnostic changes will not start
        /// renegotiation.
        #[test]
        fn negotiation_state_agnostic_change_dont_trigger_negotiation() {
            let track_patch = TrackPatchEvent {
                id: TrackId(0),
                muted: Some(true),
                enabled_individual: None,
                enabled_general: None,
            };
            let changes = vec![PeerChange::TrackPatch(track_patch.clone())];

            let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
            negotiation_sub.expect_force_update().times(1).return_once(
                move |peer_id: PeerId, updates: Vec<PeerUpdate>| {
                    assert_eq!(peer_id, PeerId(0));
                    assert_eq!(updates, vec![PeerUpdate::Updated(track_patch)]);
                },
            );
            let mut peer = Peer::new(
                PeerId(0),
                MemberId::from("member-1"),
                PeerId(1),
                MemberId::from("member-2"),
                false,
                Rc::new(negotiation_sub),
            );
            peer.add_ice_users(vec![IceUser::new_coturn_static(
                String::new(),
                String::new(),
                String::new(),
            )]);
            peer.set_initialized();
            peer.context.peer_changes_queue = changes;
            peer.commit_scheduled_changes();
        }

        /// Checks that [`PeerChange`] which doesn't requires negotiation with
        /// [`PeerChange`] which requires negotiation will trigger negotiation.
        #[test]
        fn mixed_changeset_will_trigger_negotiation() {
            let changes = vec![
                PeerChange::TrackPatch(TrackPatchEvent {
                    id: TrackId(0),
                    muted: Some(true),
                    enabled_individual: None,
                    enabled_general: None,
                }),
                PeerChange::TrackPatch(TrackPatchEvent {
                    id: TrackId(1),
                    muted: None,
                    enabled_individual: Some(true),
                    enabled_general: Some(true),
                }),
            ];

            let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
            negotiation_sub
                .expect_negotiation_needed()
                .times(1)
                .return_once(move |peer_id: PeerId| {
                    assert_eq!(peer_id, PeerId(0));
                });

            let mut peer = Peer::new(
                PeerId(0),
                MemberId::from("member-1"),
                PeerId(1),
                MemberId::from("member-2"),
                false,
                Rc::new(negotiation_sub),
            );
            peer.add_ice_users(vec![IceUser::new_coturn_static(
                String::new(),
                String::new(),
                String::new(),
            )]);
            peer.set_initialized();

            peer.context.peer_changes_queue = changes.clone();
            peer.commit_scheduled_changes();

            let reversed_changes = {
                let mut changes = changes;
                changes.reverse();
                changes
            };
            assert_eq!(peer.context.pending_peer_changes, reversed_changes);
        }

        /// Checks that [`PeerChange`] which changes negotiationless property
        /// and negotiationful property will trigger negotiation.
        #[test]
        fn mixed_change_will_trigger_negotiation() {
            let changes = vec![PeerChange::TrackPatch(TrackPatchEvent {
                id: TrackId(0),
                muted: Some(true),
                enabled_individual: Some(true),
                enabled_general: Some(true),
            })];

            let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
            negotiation_sub
                .expect_negotiation_needed()
                .times(1)
                .return_once(move |peer_id: PeerId| {
                    assert_eq!(peer_id, PeerId(0));
                });

            let mut peer = Peer::new(
                PeerId(0),
                MemberId::from("member-1"),
                PeerId(1),
                MemberId::from("member-2"),
                false,
                Rc::new(negotiation_sub),
            );
            peer.add_ice_users(vec![IceUser::new_coturn_static(
                String::new(),
                String::new(),
                String::new(),
            )]);
            peer.set_initialized();

            peer.context.peer_changes_queue = changes.clone();
            peer.commit_scheduled_changes();

            assert_eq!(peer.context.pending_peer_changes, changes);
        }
    }

    /// Checks that [`state::Peer`] generation works correctly.
    mod state_generation {
        use std::convert::TryInto;

        use super::*;

        fn peer() -> Peer<Stable> {
            let mut negotiation_sub = MockPeerUpdatesSubscriber::new();
            negotiation_sub
                .expect_negotiation_needed()
                .returning(|_: PeerId| ());
            negotiation_sub
                .expect_force_update()
                .returning(|_: PeerId, _: Vec<PeerUpdate>| ());

            let mut peer = Peer::new(
                PeerId(0),
                MemberId::from("member-1"),
                PeerId(0),
                MemberId::from("member-2"),
                false,
                Rc::new(negotiation_sub),
            );
            peer.add_ice_users(vec![IceUser::new_coturn_static(
                String::new(),
                String::new(),
                String::new(),
            )]);
            peer.set_initialized();

            peer
        }

        #[test]
        fn offerer_role() {
            let peer = peer();

            let peer = peer.start_as_offerer();
            let peer = PeerStateMachine::from(peer);
            let state = peer.get_state();
            assert_eq!(state.negotiation_role, Some(NegotiationRole::Offerer));

            let peer: Peer<WaitLocalSdp> = peer.try_into().unwrap();
            let peer = peer.set_local_offer(String::new());
            let peer = PeerStateMachine::from(peer);
            let state = peer.get_state();
            assert_eq!(state.negotiation_role, Some(NegotiationRole::Offerer));

            let peer: Peer<WaitRemoteSdp> = peer.try_into().unwrap();
            let peer = peer.set_remote_answer(String::new());
            let peer = PeerStateMachine::from(peer);
            let state = peer.get_state();

            assert_eq!(state.negotiation_role, None);
        }

        #[test]
        fn answerer_role() {
            let peer = peer();

            let peer = peer.start_as_answerer();
            let peer = PeerStateMachine::from(peer);
            let state = peer.get_state();
            assert_eq!(state.negotiation_role, None);

            let peer: Peer<WaitRemoteSdp> = peer.try_into().unwrap();
            let peer = peer.set_remote_offer(String::from("SDP"));
            let peer = PeerStateMachine::from(peer);
            let state = peer.get_state();
            assert_eq!(
                state.negotiation_role,
                Some(NegotiationRole::Answerer(String::from("SDP")))
            );

            let peer: Peer<WaitLocalSdp> = peer.try_into().unwrap();
            let peer = peer.set_local_answer(String::new());
            let peer = PeerStateMachine::from(peer);
            let state = peer.get_state();
            assert_eq!(state.negotiation_role, None);
        }

        #[test]
        fn ice_restart() {
            let mut peer = peer();

            peer.as_changes_scheduler().restart_ice();
            peer.commit_scheduled_changes();

            let peer = PeerStateMachine::from(peer);
            let state = peer.get_state();
            assert!(state.restart_ice);
        }

        #[test]
        fn sender_patch() {
            let mut peer = peer();
            peer.context.senders.insert(
                TrackId(0),
                Rc::new(MediaTrack::new(
                    TrackId(0),
                    MediaType::Audio(AudioSettings { required: true }),
                )),
            );

            peer.as_changes_scheduler()
                .patch_tracks(vec![TrackPatchCommand {
                    id: TrackId(0),
                    muted: Some(true),
                    enabled: Some(false),
                }]);

            peer.commit_scheduled_changes();

            let peer = PeerStateMachine::from(peer);
            let state = peer.get_state();
            let track_state = state.senders.get(&TrackId(0)).unwrap();
            assert_eq!(track_state.muted, true);
            assert_eq!(track_state.enabled_general, false);
            assert_eq!(track_state.enabled_individual, false);
        }

        #[test]
        fn receiver_patch() {
            let mut peer = peer();
            peer.context.receivers.insert(
                TrackId(0),
                Rc::new(MediaTrack::new(
                    TrackId(0),
                    MediaType::Audio(AudioSettings { required: true }),
                )),
            );

            peer.as_changes_scheduler()
                .patch_tracks(vec![TrackPatchCommand {
                    id: TrackId(0),
                    muted: Some(true),
                    enabled: Some(false),
                }]);

            peer.commit_scheduled_changes();

            let peer = PeerStateMachine::from(peer);
            let state = peer.get_state();
            let track_state = state.receivers.get(&TrackId(0)).unwrap();
            assert_eq!(track_state.muted, true);
            assert_eq!(track_state.enabled_general, false);
            assert_eq!(track_state.enabled_individual, false);
        }
    }
}
