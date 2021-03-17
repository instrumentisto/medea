//! Client API protocol implementation for Medea media server.
//!
//! # Features
//!
//! - `jason`: Enables [`Deserialize`] implementation for [`Event`]s, and
//! [`Serialize`] implementation for [`Command`]s.
//! - `medea`: Enables [`Deserialize`] implementation for [`Command`]s, and
//! [`Serialize`] implementation for [`Event`]s.
//! - `extended-stats`: Enables unused RTC Stats DTOs.
//!
//! # Contribution guide
//!
//! Avoid using 64 bit types. [`medea-jason`] uses [wasm-bindgen] to interop
//! with JS, and exposing 64 bit types to JS will make [wasm-bindgen] to use
//! [BigInt64Array][2] / [BigUint64Array][3] in its JS glue, which are not
//! implemented or were implemented too recently in some UAs.
//!
//! So its better to keep protocol 64-bit-types-clean to avoid things breaking
//! by accident.
//!
//! [`medea-jason`]: https://docs.rs/medea-jason
//! [wasm-bindgen]: https://github.com/rustwasm/wasm-bindgen
//! [2]: https://tinyurl.com/y8bacb93
//! [3]: https://tinyurl.com/y4j3b4cs

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(broken_intra_doc_links)]
#![forbid(unsafe_code)]

pub mod state;
pub mod stats;

use std::collections::HashMap;

use derive_more::{Constructor, Display, From};
use medea_macro::dispatchable;
use serde::{Deserialize, Serialize};

use self::stats::RtcStat;

/// ID of `Room`.
#[derive(
    Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq, From, Display,
)]
#[from(forward)]
pub struct RoomId(pub String);

/// ID of `Member`.
#[derive(
    Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq, From, Display,
)]
#[from(forward)]
pub struct MemberId(pub String);

/// ID of `Peer`.
#[cfg_attr(
    feature = "medea",
    derive(Deserialize, Debug, Hash, Eq, Default, PartialEq)
)]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[derive(Clone, Copy, Display)]
pub struct PeerId(pub u32);

/// ID of `MediaTrack`.
#[cfg_attr(
    feature = "medea",
    derive(Deserialize, Debug, Hash, Eq, Default, PartialEq)
)]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[derive(Clone, Copy, Display)]
pub struct TrackId(pub u32);

/// Credential used for `Member` authentication.
#[derive(
    Clone, Debug, Deserialize, Display, Eq, From, Hash, PartialEq, Serialize,
)]
#[from(forward)]
pub struct Credential(pub String);

/// Value that is able to be incremented by `1`.
#[cfg(feature = "medea")]
pub trait Incrementable {
    /// Returns current value + 1.
    #[must_use]
    fn incr(&self) -> Self;
}

/// Implements [`Incrementable`] trait for newtype with any numeric type.
macro_rules! impl_incrementable {
    ($name:ty) => {
        impl Incrementable for $name {
            #[inline]
            fn incr(&self) -> Self {
                Self(self.0 + 1)
            }
        }
    };
}

#[cfg(feature = "medea")]
impl_incrementable!(PeerId);
#[cfg(feature = "medea")]
impl_incrementable!(TrackId);

#[cfg_attr(feature = "medea", derive(Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
#[serde(tag = "msg", content = "data")]
/// Message sent by `Media Server` to `Client`.
pub enum ServerMsg {
    /// `ping` message that `Media Server` is expected to send to `Client`
    /// periodically for probing its aliveness.
    Ping(u32),

    /// `Media Server` notifies `Client` about happened facts and it reacts on
    /// them to reach the proper state.
    Event {
        /// ID of `Room` that this [`Event`] is associated with.
        room_id: RoomId,

        /// Actual [`Event`] sent to `Client`.
        event: Event,
    },

    /// `Media Server` notifies `Client` about necessity to update its RPC
    /// settings.
    RpcSettings(RpcSettings),
}

#[cfg_attr(feature = "medea", derive(Deserialize))]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[derive(Clone, Debug, PartialEq)]
/// Message from 'Client' to 'Media Server'.
pub enum ClientMsg {
    /// `pong` message that `Client` answers with to `Media Server` in response
    /// to received [`ServerMsg::Ping`].
    Pong(u32),

    /// Request of `Client` to change the state on `Media Server`.
    Command {
        /// ID of `Room` that this [`Command`] is associated with.
        room_id: RoomId,

        /// Actual [`Command`] sent to `Media Server`.
        command: Command,
    },
}

/// RPC settings of `Client` received from `Media Server`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RpcSettings {
    /// Timeout of considering `Client` as lost by `Media Server` when it
    /// doesn't receive [`ClientMsg::Pong`].
    ///
    /// Unit: millisecond.
    pub idle_timeout_ms: u32,

    /// Interval that `Media Server` sends [`ServerMsg::Ping`] with.
    ///
    /// Unit: millisecond.
    pub ping_interval_ms: u32,
}

/// WebSocket message from Web Client to Media Server.
#[dispatchable]
#[cfg_attr(feature = "medea", derive(Deserialize))]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[serde(tag = "command", content = "data")]
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Request of `Client` to join `Room`.
    JoinRoom {
        /// ID of `Member` with which [`Credential`] `Client` want to join.
        member_id: MemberId,

        /// [`Credential`] of `Client`'s `Member`.
        credential: Credential,
    },

    /// Request of `Client` to leave `Room`.
    LeaveRoom {
        /// ID of leaving `Member`.
        member_id: MemberId,
    },

    /// Web Client sends SDP Offer.
    MakeSdpOffer {
        /// ID of the `Peer` for which Web Client sends SDP Offer.
        peer_id: PeerId,

        /// SDP Offer of the `Peer`.
        sdp_offer: String,

        /// Associations between [`Track`] and transceiver's
        /// [media description][1].
        ///
        /// `mid` is basically an ID of [`m=<media>` section][1] in SDP.
        ///
        /// [1]: https://tools.ietf.org/html/rfc4566#section-5.14
        mids: HashMap<TrackId, String>,

        /// Statuses of `Peer` transceivers.
        transceivers_statuses: HashMap<TrackId, bool>,
    },

    /// Web Client sends SDP Answer.
    MakeSdpAnswer {
        /// ID of the `Peer` for which Web Client sends SDP Answer.
        peer_id: PeerId,

        /// SDP Answer of the `Peer`.
        sdp_answer: String,

        /// Statuses of `Peer` transceivers.
        transceivers_statuses: HashMap<TrackId, bool>,
    },

    /// Web Client sends Ice Candidate.
    SetIceCandidate {
        peer_id: PeerId,
        candidate: IceCandidate,
    },

    /// Web Client sends Peer Connection metrics.
    AddPeerConnectionMetrics {
        peer_id: PeerId,
        metrics: PeerMetrics,
    },

    /// Web Client asks permission to update [`Track`]s in specified Peer.
    /// Media Server gives permission by sending [`Event::TracksApplied`].
    UpdateTracks {
        peer_id: PeerId,
        tracks_patches: Vec<TrackPatchCommand>,
    },

    /// Web Client asks Media Server to synchronize Client State with a Server
    /// State.
    SynchronizeMe { state: state::Room },
}

/// Web Client's Peer Connection metrics.
#[cfg_attr(feature = "medea", derive(Deserialize))]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum PeerMetrics {
    /// Peer Connection's ICE connection state.
    IceConnectionState(IceConnectionState),

    /// Peer Connection's connection state.
    PeerConnectionState(PeerConnectionState),

    /// Peer Connection's RTC stats.
    RtcStats(Vec<RtcStat>),
}

/// Peer Connection's ICE connection state.
#[cfg_attr(feature = "medea", derive(Deserialize))]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum IceConnectionState {
    /// ICE agent is gathering addresses or is waiting to be given remote
    /// candidates.
    New,

    /// ICE agent has been given one or more remote candidates and is checking
    /// pairs of local and remote candidates against one another to try to find
    /// a compatible match, but hasn't yet found a pair which will allow the
    /// `PeerConnection` to be made. It's possible that gathering of candidates
    /// is also still underway.
    Checking,

    /// Usable pairing of local and remote candidates has been found for all
    /// components of the connection, and the connection has been established.
    /// It's possible that gathering is still underway, and it's also possible
    /// that the ICE agent is still checking candidates against one another
    /// looking for a better connection to use.
    Connected,

    /// ICE agent has finished gathering candidates, has checked all pairs
    /// against one another, and has found a connection for all components.
    Completed,

    /// ICE candidate has checked all candidates pairs against one another and
    /// has failed to find compatible matches for all components of the
    /// connection. It is, however, possible that the ICE agent did find
    /// compatible connections for some components.
    Failed,

    /// Checks to ensure that components are still connected failed for at
    /// least one component of the `PeerConnection`. This is a less stringent
    /// test than [`IceConnectionState::Failed`] and may trigger intermittently
    /// and resolve just as spontaneously on less reliable networks, or during
    /// temporary disconnections. When the problem resolves, the connection may
    /// return to the [`IceConnectionState::Connected`] state.
    Disconnected,

    /// ICE agent for this `PeerConnection` has shut down and is no longer
    /// handling requests.
    Closed,
}

/// Peer Connection's connection state.
#[cfg_attr(feature = "medea", derive(Deserialize))]
#[cfg_attr(feature = "jason", derive(Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerConnectionState {
    /// At least one of the connection's ICE transports are in the
    /// [`IceConnectionState::New`] state, and none of them are in one
    /// of the following states: [`IceConnectionState::Checking`],
    /// [`IceConnectionState::Failed`], or
    /// [`IceConnectionState::Disconnected`], or all of the connection's
    /// transports are in the [`IceConnectionState::Closed`] state.
    New,

    /// One or more of the ICE transports are currently in the process of
    /// establishing a connection; that is, their [`IceConnectionState`] is
    /// either [`IceConnectionState::Checking`] or
    /// [`IceConnectionState::Connected`], and no transports are in the
    /// [`IceConnectionState::Failed`] state.
    Connecting,

    /// Every ICE transport used by the connection is either in use (state
    /// [`IceConnectionState::Connected`] or [`IceConnectionState::Completed`])
    /// or is closed ([`IceConnectionState::Closed`]); in addition,
    /// at least one transport is either [`IceConnectionState::Connected`] or
    /// [`IceConnectionState::Completed`].
    Connected,

    /// At least one of the ICE transports for the connection is in the
    /// [`IceConnectionState::Disconnected`] state and none of the other
    /// transports are in the state [`IceConnectionState::Failed`] or
    /// [`IceConnectionState::Checking`].
    Disconnected,

    /// One or more of the ICE transports on the connection is in the
    /// [`IceConnectionState::Failed`] state.
    Failed,

    /// The `PeerConnection` is closed.
    Closed,
}

impl From<IceConnectionState> for PeerConnectionState {
    fn from(ice_con_state: IceConnectionState) -> Self {
        use IceConnectionState as IceState;

        match ice_con_state {
            IceState::New => Self::New,
            IceState::Checking => Self::Connecting,
            IceState::Connected | IceState::Completed => Self::Connected,
            IceState::Failed => Self::Failed,
            IceState::Disconnected => Self::Disconnected,
            IceState::Closed => Self::Closed,
        }
    }
}

/// Reason of disconnecting Web Client from Media Server.
#[derive(
    Copy, Clone, Debug, Deserialize, Display, Serialize, Eq, PartialEq,
)]
pub enum CloseReason {
    /// Client session was finished on a server side.
    Finished,

    /// Old connection was closed due to a client reconnection.
    Reconnected,

    /// Connection has been inactive for a while and thus considered idle
    /// by a server.
    Idle,

    /// Establishing of connection with a server was rejected on server side.
    ///
    /// Most likely because of incorrect Member credentials.
    Rejected,

    /// Server internal error has occurred while connecting.
    ///
    /// This close reason is similar to 500 HTTP status code.
    InternalError,

    /// Client was evicted on the server side.
    Evicted,
}

/// Description which is sent in [Close] WebSocket frame from Media Server
/// to Web Client.
///
/// [Close]: https://tools.ietf.org/html/rfc6455#section-5.5.1
#[derive(Constructor, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloseDescription {
    /// Reason of why WebSocket connection has been closed.
    pub reason: CloseReason,
}

/// WebSocket message from Medea to Jason.
#[dispatchable(self: &Self, async_trait(?Send))]
#[cfg_attr(feature = "medea", derive(Clone, Debug, Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[serde(tag = "event", content = "data")]
pub enum Event {
    /// `Media Server` notifies `Client` that he joined `Room`.
    RoomJoined {
        /// ID of `Member` which joined `Room`.
        member_id: MemberId,
    },

    /// `Media Server` notifies `Client` that he left `Room`.
    RoomLeft {
        /// [`CloseReason`] with which `Client` was left.
        close_reason: CloseReason,
    },

    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// creation.
    PeerCreated {
        peer_id: PeerId,
        negotiation_role: NegotiationRole,
        tracks: Vec<Track>,
        ice_servers: Vec<IceServer>,
        force_relay: bool,
    },

    /// Media Server notifies Web Client about necessity to apply specified SDP
    /// Answer to Web Client's RTCPeerConnection.
    SdpAnswerMade { peer_id: PeerId, sdp_answer: String },

    /// Media Server notifies Web Client that his SDP offer was applied.
    LocalDescriptionApplied { peer_id: PeerId, sdp_offer: String },

    /// Media Server notifies Web Client about necessity to apply specified
    /// ICE Candidate.
    IceCandidateDiscovered {
        peer_id: PeerId,
        candidate: IceCandidate,
    },

    /// Media Server notifies Web Client about necessity of RTCPeerConnection
    /// close.
    PeersRemoved { peer_ids: Vec<PeerId> },

    /// Media Server notifies about necessity to update [`Track`]s in specified
    /// `Peer`.
    TracksApplied {
        /// [`PeerId`] of `Peer` where [`Track`]s should be updated.
        peer_id: PeerId,

        /// List of [`TrackUpdate`]s which should be applied.
        updates: Vec<TrackUpdate>,

        /// Negotiation role basing on which should be sent
        /// [`Command::MakeSdpOffer`] or [`Command::MakeSdpAnswer`].
        ///
        /// If `None` then no (re)negotiation should be done.
        negotiation_role: Option<NegotiationRole>,
    },

    /// Media Server notifies about connection quality score update.
    ConnectionQualityUpdated {
        /// Partner [`MemberId`] of the `Peer`.
        partner_member_id: MemberId,

        /// Estimated connection quality.
        quality_score: ConnectionQualityScore,
    },

    /// Media Server synchronizes Web Client about State synchronization.
    StateSynchronized { state: state::Room },
}

/// `Peer`'s negotiation role.
///
/// Some [`Event`]s can trigger SDP negotiation.
/// - If [`Event`] contains [`NegotiationRole::Offerer`], then `Peer` is
///   expected to create SDP Offer and send it via [`Command::MakeSdpOffer`].
/// - If [`Event`] contains [`NegotiationRole::Answerer`], then `Peer` is
///   expected to apply provided SDP Offer and provide its SDP Answer in a
///   [`Command::MakeSdpAnswer`].
#[cfg_attr(feature = "medea", derive(Clone, Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[derive(Debug)]
pub enum NegotiationRole {
    /// [`Command::MakeSdpOffer`] should be sent by client.
    Offerer,

    /// [`Command::MakeSdpAnswer`] should be sent by client.
    Answerer(String),
}

/// [`Track`] update which should be applied to the `Peer`.
#[cfg_attr(feature = "medea", derive(Clone, Debug, Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
pub enum TrackUpdate {
    /// New [`Track`] should be added to the `Peer`.
    Added(Track),

    /// `Track` with a provided [`TrackId`] should be removed from the `Peer`.
    ///
    /// Can only refer tracks already known to the `Peer`.
    Removed(TrackId),

    /// [`Track`] should be updated by this [`TrackPatchEvent`] in the `Peer`.
    /// Can only refer tracks already known to the `Peer`.
    Updated(TrackPatchEvent),

    /// `Peer` should start ICE restart process on the next renegotiation.
    IceRestart,
}

/// Represents [RTCIceCandidateInit][1] object.
///
/// [1]: https://www.w3.org/TR/webrtc/#dom-rtcicecandidateinit
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_m_line_index: Option<u16>,
    pub sdp_mid: Option<String>,
}

/// [`Track`] with specified direction.
#[cfg_attr(feature = "medea", derive(Clone, Debug, Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
pub struct Track {
    pub id: TrackId,
    pub direction: Direction,
    pub media_type: MediaType,
}

impl Track {
    /// Indicates whether this [`Track`] is required to call starting.
    #[must_use]
    pub fn required(&self) -> bool {
        self.media_type.required()
    }
}

/// Patch of the [`Track`] which Web Client can request with
/// [`Command::UpdateTracks`].
#[cfg_attr(feature = "medea", derive(Clone, Debug, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[derive(Eq, PartialEq)]
pub struct TrackPatchCommand {
    pub id: TrackId,
    pub enabled: Option<bool>,
    pub muted: Option<bool>,
}

/// Patch of the [`Track`] which Media Server can send with
/// [`Event::TracksApplied`].
#[cfg_attr(feature = "medea", derive(Clone, Debug, Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
pub struct TrackPatchEvent {
    /// ID of the [`Track`] which should be patched.
    pub id: TrackId,

    /// Media exchange state of the concrete `Member`.
    ///
    /// This state doesn't indicates that connection between two `Member`s are
    /// really disabled. This is intention of this `Member`.
    pub enabled_individual: Option<bool>,

    /// Media exchange state of the connection between `Member`s.
    ///
    /// This state indicates real media exchange state between `Member`s. But
    /// this state doesn't changes intention of this `Member`.
    ///
    /// So intention of this `Member` (`enabled_individual`) can be
    /// `false`, but real media exchange state can be `true`.
    pub enabled_general: Option<bool>,

    /// `Track` mute state. Muting and unmuting can be performed without adding
    /// / removing tracks from transceivers, hence renegotiation is not
    /// required.
    pub muted: Option<bool>,
}

impl From<TrackPatchCommand> for TrackPatchEvent {
    fn from(from: TrackPatchCommand) -> Self {
        Self {
            id: from.id,
            enabled_individual: from.enabled,
            enabled_general: None,
            muted: from.muted,
        }
    }
}

impl TrackPatchEvent {
    /// Returns new empty [`TrackPatchEvent`] with a provided [`TrackId`].
    #[inline]
    #[must_use]
    pub fn new(id: TrackId) -> Self {
        Self {
            id,
            enabled_general: None,
            enabled_individual: None,
            muted: None,
        }
    }

    /// Merges this [`TrackPatchEvent`] with a provided [`TrackPatchEvent`].
    ///
    /// Does nothing if [`TrackId`] of this [`TrackPatchEvent`] and the
    /// provided [`TrackPatchEvent`] are different.
    pub fn merge(&mut self, another: &Self) {
        if self.id != another.id {
            return;
        }

        if let Some(enabled_general) = another.enabled_general {
            self.enabled_general = Some(enabled_general);
        }

        if let Some(enabled_individual) = another.enabled_individual {
            self.enabled_individual = Some(enabled_individual);
        }

        if let Some(muted) = another.muted {
            self.muted = Some(muted);
        }
    }
}

/// Representation of [RTCIceServer][1] (item of `iceServers` field
/// from [RTCConfiguration][2]).
///
/// [1]: https://developer.mozilla.org/en-US/docs/Web/API/RTCIceServer
/// [2]: https://developer.mozilla.org/en-US/docs/Web/API/RTCConfiguration
#[derive(Clone, Debug)]
#[cfg_attr(feature = "medea", derive(Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

/// Direction of [`Track`].
#[cfg_attr(feature = "medea", derive(Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[derive(Clone, Debug)]
// TODO: Use different struct without mids in TracksApplied event.
pub enum Direction {
    Send {
        receivers: Vec<MemberId>,
        mid: Option<String>,
    },
    Recv {
        sender: MemberId,
        mid: Option<String>,
    },
}

/// Type of [`Track`].
#[cfg_attr(feature = "medea", derive(Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[derive(Clone, Debug)]
pub enum MediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

impl MediaType {
    /// Returns `true` if this [`MediaType`] is required to call starting.
    #[must_use]
    pub fn required(&self) -> bool {
        match self {
            MediaType::Audio(audio) => audio.required,
            MediaType::Video(video) => video.required,
        }
    }
}

#[cfg_attr(feature = "medea", derive(Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[derive(Clone, Debug)]
pub struct AudioSettings {
    /// Importance of the audio media type.
    ///
    /// If `false` then audio may be not published.
    pub required: bool,
}

#[cfg_attr(feature = "medea", derive(Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[derive(Clone, Debug)]
pub struct VideoSettings {
    /// Importance of the video media type.
    ///
    /// If `false` then video may be not published.
    pub required: bool,

    /// Source kind of this [`VideoSettings`] media.
    pub source_kind: MediaSourceKind,
}

/// Media source kind.
#[cfg_attr(feature = "medea", derive(Debug, Eq, PartialEq, Serialize))]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[derive(Clone, Copy)]
pub enum MediaSourceKind {
    /// Media is sourced by some media device (webcam or microphone).
    Device,

    /// Media is obtained with screen-capture.
    Display,
}

/// Estimated connection quality.
#[cfg_attr(
    feature = "medea",
    derive(Serialize, Display, Eq, Ord, PartialEq, PartialOrd)
)]
#[cfg_attr(feature = "jason", derive(Deserialize))]
#[derive(Clone, Copy, Debug)]
pub enum ConnectionQualityScore {
    /// Nearly all users dissatisfied.
    Poor = 1,

    /// Many users dissatisfied.
    Low = 2,

    /// Some users dissatisfied.
    Medium = 3,

    /// Satisfied.
    High = 4,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn track_patch_merge() {
        for (track_patches, result) in vec![
            (
                vec![
                    TrackPatchEvent {
                        id: TrackId(1),
                        enabled_general: Some(true),
                        enabled_individual: Some(true),
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
                        id: TrackId(1),
                        enabled_general: Some(true),
                        enabled_individual: Some(true),
                        muted: None,
                    },
                    TrackPatchEvent {
                        id: TrackId(1),
                        enabled_general: Some(true),
                        enabled_individual: Some(true),
                        muted: None,
                    },
                ],
                TrackPatchEvent {
                    id: TrackId(1),
                    enabled_general: Some(true),
                    enabled_individual: Some(true),
                    muted: None,
                },
            ),
            (
                vec![
                    TrackPatchEvent {
                        id: TrackId(1),
                        enabled_general: None,
                        enabled_individual: None,
                        muted: None,
                    },
                    TrackPatchEvent {
                        id: TrackId(1),
                        enabled_general: Some(true),
                        enabled_individual: Some(true),
                        muted: None,
                    },
                ],
                TrackPatchEvent {
                    id: TrackId(1),
                    enabled_general: Some(true),
                    enabled_individual: Some(true),
                    muted: None,
                },
            ),
            (
                vec![
                    TrackPatchEvent {
                        id: TrackId(1),
                        enabled_general: Some(true),
                        enabled_individual: Some(true),
                        muted: None,
                    },
                    TrackPatchEvent {
                        id: TrackId(1),
                        enabled_general: None,
                        enabled_individual: None,
                        muted: None,
                    },
                ],
                TrackPatchEvent {
                    id: TrackId(1),
                    enabled_general: Some(true),
                    enabled_individual: Some(true),
                    muted: None,
                },
            ),
            (
                vec![
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
                ],
                TrackPatchEvent {
                    id: TrackId(1),
                    enabled_general: None,
                    enabled_individual: None,
                    muted: None,
                },
            ),
        ] {
            let mut merge_track_patch = TrackPatchEvent::new(TrackId(1));
            for track_patch in &track_patches {
                merge_track_patch.merge(track_patch);
            }

            assert_eq!(
                result, merge_track_patch,
                "track patches: {:?}",
                track_patches
            );
        }
    }
}
