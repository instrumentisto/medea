//! Contains DTOs for [RTCPeerConnection] metrics.
//!
//! [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection

#![allow(clippy::module_name_repetitions)]

use std::{
    hash::{Hash, Hasher},
    time::{Duration, SystemTime},
};

use derive_more::{Display, From};
use serde::{Deserialize, Serialize};

/// Enum with which you can try to deserialize some known enum and if it
/// isn't known, then unknown data will be stored as [`String`] in the
/// [`NonExhaustive::Unknown`] variant.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(untagged)]
pub enum NonExhaustive<T> {
    /// Will store known enum variant if it successfully deserialized.
    Known(T),

    /// Will store unknown enum variant with it's data as [`String`].
    Unknown(String),
}

/// Unique ID that is associated with the object that was inspected to produce
/// [`RtcStat`] object.
///
/// Two [`RtcStat`]s objects, extracted from two different [RTCStatsReport]
/// objects, MUST have the same ID if they were produced by inspecting the same
/// underlying object.
///
/// [RTCStatsReport]: https://w3.org/TR/webrtc/#dom-rtcstatsreport
#[derive(
    Clone, Debug, Deserialize, Display, Eq, From, Hash, PartialEq, Serialize,
)]
#[from(forward)]
pub struct StatId(pub String);

/// Represents the [stats object] constructed by inspecting a specific
/// [monitored object].
///
/// [Full doc on W3C][1].
///
/// [stats object]: https://w3.org/TR/webrtc-stats/#dfn-stats-object
/// [monitored object]: https://w3.org/TR/webrtc-stats/#dfn-monitored-object
/// [1]: https://w3.org/TR/webrtc/#rtcstats-dictionary
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
pub struct RtcStat {
    /// Unique ID that is associated with the object that was inspected to
    /// produce this [RTCStats] object.
    ///
    /// [RTCStats]: https://w3.org/TR/webrtc/#dom-rtcstats
    pub id: StatId,

    /// Timestamp associated with this object.
    ///
    /// The time is relative to the UNIX epoch (Jan 1, 1970, UTC).
    ///
    /// For statistics that came from a remote source (e.g., from received RTCP
    /// packets), timestamp represents the time at which the information
    /// arrived at the local endpoint. The remote timestamp can be found in an
    /// additional field in an [`RtcStat`]-derived dictionary, if applicable.
    pub timestamp: HighResTimeStamp,

    /// Actual stats of this [`RtcStat`].
    ///
    /// All possible stats are described in the [`RtcStatsType`] enum.
    #[serde(flatten)]
    pub stats: RtcStatsType,
}

/// All known types of [`RtcStat`]s.
///
/// [List of all RTCStats types on W3C][1].
///
/// [1]: https://w3.org/TR/webrtc-stats/#rtctatstype-%2A
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum RtcStatsType {
    /// Statistics for a codec that is currently used by [RTP] streams being
    /// sent or received by [RTCPeerConnection] object.
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    #[cfg(feature = "extended-stats")]
    Codec(Box<RtcCodecStats>),

    /// Statistics for an inbound [RTP] stream that is currently received with
    /// [RTCPeerConnection] object.
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    InboundRtp(Box<RtcInboundRtpStreamStats>),

    /// Statistics for an outbound [RTP] stream that is currently sent with
    /// [RTCPeerConnection] object.
    ///
    /// When there are multiple [RTP] streams connected to the same sender,
    /// such as when using simulcast or RTX, there will be one
    /// [`RtcOutboundRtpStreamStats`] per RTP stream, with distinct values of
    /// the `ssrc` attribute, and all these senders will have a reference to
    /// the same "sender" object (of type [RTCAudioSenderStats][1] or
    /// [RTCVideoSenderStats][2]) and "track" object (of type
    /// [RTCSenderAudioTrackAttachmentStats][3] or
    /// [RTCSenderVideoTrackAttachmentStats][4]).
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    /// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcaudiosenderstats
    /// [2]: https://w3.org/TR/webrtc-stats/#dom-rtcvideosenderstats
    /// [3]: https://tinyurl.com/sefa5z4
    /// [4]: https://tinyurl.com/rkuvpl4
    OutboundRtp(Box<RtcOutboundRtpStreamStats>),

    /// Statistics for the remote endpoint's inbound [RTP] stream corresponding
    /// to an outbound stream that is currently sent with [RTCPeerConnection]
    /// object.
    ///
    /// It is measured at the remote endpoint and reported in a RTCP Receiver
    /// Report (RR) or RTCP Extended Report (XR).
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    RemoteInboundRtp(Box<RtcRemoteInboundRtpStreamStats>),

    /// Statistics for the remote endpoint's outbound [RTP] stream
    /// corresponding to an inbound stream that is currently received with
    /// [RTCPeerConnection] object.
    ///
    /// It is measured at the remote endpoint and reported in an RTCP Sender
    /// Report (SR).
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    RemoteOutboundRtp(Box<RtcRemoteOutboundRtpStreamStats>),

    /// Statistics for the media produced by a [MediaStreamTrack][1] that is
    /// currently attached to an [RTCRtpSender]. This reflects the media that
    /// is fed to the encoder after [getUserMedia] constraints have been
    /// applied (i.e. not the raw media produced by the camera).
    ///
    /// [RTCRtpSender]: https://w3.org/TR/webrtc/#rtcrtpsender-interface
    /// [getUserMedia]: https://tinyurl.com/sngpyr6
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    MediaSource(Box<MediaSourceStats>),

    /// Statistics for a contributing source (CSRC) that contributed to an
    /// inbound [RTP] stream.
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    #[cfg(feature = "extended-stats")]
    Csrc(Box<RtpContributingSourceStats>),

    /// Statistics related to the [RTCPeerConnection] object.
    ///
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    #[cfg(feature = "extended-stats")]
    PeerConnection(Box<RtcPeerConnectionStats>),

    /// Statistics related to each [RTCDataChannel] ID.
    ///
    /// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
    #[cfg(feature = "extended-stats")]
    DataChannel(Box<DataChannelStats>),

    /// Contains statistics related to a specific [MediaStream].
    ///
    /// This is now obsolete.
    ///
    /// [MediaStream]: https://w3.org/TR/mediacapture-streams/#mediastream
    #[cfg(feature = "extended-stats")]
    Stream(Box<MediaStreamStats>),

    /// Statistics related to a specific [MediaStreamTrack][1]'s attachment to
    /// an [RTCRtpSender] and the corresponding media-level metrics.
    ///
    /// [RTCRtpSender]: https://w3.org/TR/webrtc/#rtcrtpsender-interface
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    Track(Box<TrackStats>),

    /// Statistics related to a specific [RTCRtpTransceiver].
    ///
    /// [RTCRtpTransceiver]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver
    #[cfg(feature = "extended-stats")]
    Transceiver(Box<RtcRtpTransceiverStats>),

    /// Statistics related to a specific [RTCRtpSender] and the corresponding
    /// media-level metrics.
    ///
    /// [RTCRtpSender]: https://w3.org/TR/webrtc/#rtcrtpsender-interface
    #[cfg(feature = "extended-stats")]
    Sender(Box<SenderStatsKind>),

    /// Statistics related to a specific [RTCRtpReceiver] and the corresponding
    /// media-level metrics.
    ///
    /// [RTCRtpReceiver]: https://w3.org/TR/webrtc/#dom-rtcrtpreceiver
    #[cfg(feature = "extended-stats")]
    Receiver(Box<ReceiverStatsKind>),

    /// Transport statistics related to the [RTCPeerConnection] object.
    ///
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    Transport(Box<RtcTransportStats>),

    /// SCTP transport statistics related to an [RTCSctpTransport] object.
    ///
    /// [RTCSctpTransport]: https://w3.org/TR/webrtc/#dom-rtcsctptransport
    SctpTransport(Box<RtcSctpTransportStats>),

    /// ICE candidate pair statistics related to the [RTCIceTransport] objects.
    ///
    /// A candidate pair that is not the current pair for a transport is
    /// [deleted][1] when the [RTCIceTransport] does an ICE restart, at the
    /// time the state changes to `new`.
    ///
    /// The candidate pair that is the current pair for a transport is deleted
    /// after an ICE restart when the [RTCIceTransport] switches to using a
    /// candidate pair generated from the new candidates; this time doesn't
    /// correspond to any other externally observable event.
    ///
    /// [RTCIceTransport]: https://w3.org/TR/webrtc/#dom-rtcicetransport
    /// [1]: https://w3.org/TR/webrtc-stats/#dfn-deleted
    CandidatePair(Box<RtcIceCandidatePairStats>),

    /// ICE local candidate statistics related to the [RTCIceTransport]
    /// objects.
    ///
    /// A local candidate is [deleted][1] when the [RTCIceTransport] does an
    /// ICE restart, and the candidate is no longer a member of any
    /// non-deleted candidate pair.
    ///
    /// [RTCIceTransport]: https://w3.org/TR/webrtc/#dom-rtcicetransport
    /// [1]: https://w3.org/TR/webrtc-stats/#dfn-deleted
    LocalCandidate(Box<RtcIceCandidateStats>),

    /// ICE remote candidate statistics related to the [RTCIceTransport]
    /// objects.
    ///
    /// A remote candidate is [deleted][1] when the [RTCIceTransport] does an
    /// ICE restart, and the candidate is no longer a member of any non-deleted
    /// candidate pair.
    ///
    /// [RTCIceTransport]: https://w3.org/TR/webrtc/#dom-rtcicetransport
    /// [1]: https://w3.org/TR/webrtc-stats/#dfn-deleted
    RemoteCandidate(Box<RtcIceCandidateStats>),

    /// Information about a certificate used by [RTCIceTransport].
    ///
    /// [RTCIceTransport]: https://w3.org/TR/webrtc/#dom-rtcicetransport
    #[cfg(feature = "extended-stats")]
    Certificate(Box<RtcCertificateStats>),

    /// Information about the connection to an ICE server (e.g. STUN or TURN).
    #[cfg(feature = "extended-stats")]
    IceServer(Box<RtcIceServerStats>),

    /// Disabled or unknown variants of stats will be deserialized as
    /// [`RtcStatsType::Other`].
    #[serde(other)]
    Other,
}

#[cfg(feature = "extended-stats")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-stats")))]
/// Contains statistics related to a specific [MediaStream].
///
/// This is now obsolete.
///
/// [`RtcStatsType::Stream`] variant.
///
/// [Full doc on W3C][1].
///
/// [MediaStream]: https://w3.org/TR/mediacapture-streams/#mediastream
/// [1]: https://w3.org/TR/webrtc-stats/#idl-def-rtcmediastreamstats
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaStreamStats {
    /// [`stream.id`][1] property.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastream-id
    pub stream_identifier: String,

    /// ID of the stats object, not the `track.id`.
    pub track_ids: Vec<StatId>,
}

#[cfg(feature = "extended-stats")]
/// Statistics related to each [RTCDataChannel] ID.
///
/// [`RtcStatsType::DataChannel`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
/// [1]: https://w3.org/TR/webrtc-stats/#dcstats-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataChannelStats {
    /// [`label`][1] value of the [RTCDataChannel] object.
    ///
    /// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
    /// [1]: https://w3.org/TR/webrtc/#dom-datachannel-label
    pub label: Option<String>,

    /// [`protocol`][1] value of the [RTCDataChannel] object.
    ///
    /// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
    /// [1]: https://w3.org/TR/webrtc/#dom-datachannel-protocol
    pub protocol: Option<Protocol>,

    /// [`id`][1] attribute of the [RTCDataChannel] object.
    ///
    /// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcdatachannel-id
    pub data_channel_identifier: Option<u64>,

    /// [Stats object reference][1] for the transport used to carry
    /// [RTCDataChannel].
    ///
    /// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
    /// [1]: https://w3.org/TR/webrtc-stats/#dfn-stats-object-reference
    pub transport_id: Option<String>,

    /// [`readyState`][1] value of the [RTCDataChannel] object.
    ///
    /// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
    /// [1]: https://w3.org/TR/webrtc/#dom-datachannel-readystate
    pub state: Option<DataChannelState>,

    /// Total number of API `message` events sent.
    pub messages_sent: Option<u64>,

    /// Total number of payload bytes sent on this [RTCDataChannel], i.e. not
    /// including headers or padding.
    ///
    /// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
    pub bytes_sent: Option<u64>,

    /// Total number of API `message` events received.
    pub messages_received: Option<u64>,

    /// Total number of bytes received on this [RTCDataChannel], i.e. not
    /// including headers or padding.
    ///
    /// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
    pub bytes_received: Option<u64>,
}

/// Non-exhaustive version of [`KnownDataChannelState`].
pub type DataChannelState = NonExhaustive<KnownDataChannelState>;

/// State of the [RTCDataChannel]'s underlying data connection.
///
/// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownDataChannelState {
    /// User agent is attempting to establish the underlying data transport.
    /// This is the initial state of [RTCDataChannel] object, whether created
    /// with [createDataChannel][1], or dispatched as a part of an
    /// [RTCDataChannelEvent].
    ///
    /// [RTCDataChannel]: https://w3.org/TR/webrtc/#dom-rtcdatachannel
    /// [RTCDataChannelEvent]: https://w3.org/TR/webrtc/#dom-rtcdatachannelevent
    /// [1]: https://w3.org/TR/webrtc/#dom-peerconnection-createdatachannel
    Connecting,

    /// [Underlying data transport][1] is established and communication is
    /// possible.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dfn-data-transport
    Open,

    /// [`procedure`][2] to close down the [underlying data transport][1] has
    /// started.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dfn-data-transport
    /// [2]: https://w3.org/TR/webrtc/#data-transport-closing-procedure
    Closing,

    /// [Underlying data transport][1] has been [`closed`][2] or could not be
    /// established.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dfn-data-transport
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcdatachannelstate-closed
    Closed,
}

#[cfg(feature = "extended-stats")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-stats")))]
/// Stats for the [RTCPeerConnection] object.
///
/// [`RtcStatsType::PeerConnection`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
/// [1]: https://w3.org/TR/webrtc-stats/#pcstats-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcPeerConnectionStats {
    /// Number of unique `DataChannel`s that have entered the `open` state
    /// during their lifetime.
    pub data_channels_opened: Option<u64>,

    /// Number of unique `DataChannel`s that have left the `open` state during
    /// their lifetime (due to being closed by either end or the underlying
    /// transport being closed). `DataChannel`s that transition from
    /// `connecting` to `closing` or `closed` without ever being `open` are not
    /// counted in this number.
    pub data_channels_closed: Option<u64>,

    /// Number of unique `DataChannel`s returned from a successful
    /// [createDataChannel][1] call on the [RTCPeerConnection].
    /// If the underlying data transport is not established, these may be in
    /// the `connecting` state.
    ///
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    /// [1]: https://w3.org/TR/webrtc/#dom-peerconnection-createdatachannel
    pub data_channels_requested: Option<u64>,

    /// Number of unique `DataChannel`s signaled in a `datachannel` event on
    /// the [RTCPeerConnection].
    ///
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    pub data_channels_accepted: Option<u64>,
}

#[cfg(feature = "extended-stats")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-stats")))]
/// Statistics for a contributing source (CSRC) that contributed to an inbound
/// [RTP] stream.
///
/// [`RtcStatsType::Csrc`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
/// [1]: https://w3.org/TR/webrtc-stats/#contributingsourcestats-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtpContributingSourceStats {
    /// SSRC identifier of the contributing source represented by the stats
    /// object, as defined by [RFC 3550]. It is a 32-bit unsigned integer that
    /// appears in the CSRC list of any packets the relevant source contributed
    /// to.
    ///
    /// [RFC 3550]: https://tools.ietf.org/html/rfc3550
    pub contributor_ssrc: Option<u32>,

    /// ID of the [RTCInboundRtpStreamStats][1] object representing the inbound
    /// [RTP] stream that this contributing source is contributing to.
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    /// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
    pub inbound_rtp_stream_id: Option<String>,

    /// Total number of [RTP] packets that this contributing source contributed
    /// to.
    ///
    /// This value is incremented each time a packet is counted by
    /// [RTCInboundRtpStreamStats.packetsReceived][2], and the packet's CSRC
    /// list (as defined by [Section 5.1 in RFC 3550][3]) contains the SSRC
    /// identifier of this contributing source, [`contributorSsrc`].
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    /// [`contributorSsrc`]: https://tinyurl.com/tf8c7j4
    /// [2]: https://tinyurl.com/rreuf49
    /// [3]: https://tools.ietf.org/html/rfc3550#section-5.1
    pub packets_contributed_to: Option<u64>,

    /// Present if the last received RTP packet that this source contributed to
    /// contained an [RFC 6465] mixer-to-client audio level header extension.
    ///
    /// The value of [`audioLevel`] is between `0..1` (linear), where `1.0`
    /// represents `0 dBov`, `0` represents silence, and `0.5` represents
    /// approximately `6 dBSPL` change in the sound pressure level from 0
    /// dBov. The [RFC 6465] header extension contains values in the range
    /// `0..127`, in units of `-dBov`, where `127` represents silence. To
    /// convert these values to the linear `0..1` range of `audioLevel`, a
    /// value of `127` is converted to `0`, and all other values are
    /// converted using the equation:
    ///
    /// `f(rfc6465_level) = 10^(-rfc6465_level/20)`
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    /// [RFC 6465]: https://tools.ietf.org/html/rfc6465
    /// [`audioLevel`]: https://tinyurl.com/sfy699q
    pub audio_level: Option<Float>,
}

/// Statistics for the remote endpoint's outbound [RTP] stream corresponding
/// to an inbound stream that is currently received with [RTCPeerConnection]
/// object.
///
/// It is measured at the remote endpoint and reported in an RTCP Sender Report
/// (SR).
///
/// [`RtcStatsType::RemoteOutboundRtp`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
/// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
/// [1]: https://w3.org/TR/webrtc-stats/#remoteoutboundrtpstats-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcRemoteOutboundRtpStreamStats {
    /// [`localId`] is used for looking up the local
    /// [RTCInboundRtpStreamStats][1] object for the same SSRC.
    ///
    /// [`localId`]: https://tinyurl.com/vu9tb2e
    /// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
    pub local_id: Option<String>,

    /// [`remoteTimestamp`] (as [HIGHRES-TIME]) is the remote timestamp at
    /// which these statistics were sent by the remote endpoint. This
    /// differs from timestamp, which represents the time at which the
    /// statistics were generated or received by the local endpoint. The
    /// [`remoteTimestamp`], if present, is derived from the NTP timestamp
    /// in an RTCP Sender Report (SR) block, which reflects the remote
    /// endpoint's clock. That clock may not be synchronized with the local
    /// clock.
    ///
    /// [`remoteTimestamp`]: https://tinyurl.com/rzlhs87
    /// [HIGRES-TIME]: https://w3.org/TR/webrtc-stats/#bib-highres-time
    pub remote_timestamp: Option<HighResTimeStamp>,

    /// Total number of RTCP SR blocks sent for this SSRC.
    pub reports_sent: Option<u64>,
}

/// Statistics for the remote endpoint's inbound [RTP] stream corresponding
/// to an outbound stream that is currently sent with [RTCPeerConnection]
/// object.
///
/// It is measured at the remote endpoint and reported in a RTCP Receiver
/// Report (RR) or RTCP Extended Report (XR).
///
/// [`RtcStatsType::RemoteInboundRtp`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
/// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
/// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcRemoteInboundRtpStreamStats {
    /// [`localId`] is used for looking up the local
    /// [RTCOutboundRtpStreamStats] object for the same SSRC.
    ///
    /// [`localId`]: https://tinyurl.com/r8uhbo9
    /// [RTCOutBoundRtpStreamStats]: https://tinyurl.com/r6f5vqg
    pub local_id: Option<String>,

    /// Packet [jitter] measured in seconds for this SSRC.
    ///
    /// [jitter]: https://en.wikipedia.org/wiki/Jitter
    pub jitter: Option<Float>,

    /// Estimated round trip time for this SSRC based on the RTCP timestamps in
    /// the RTCP Receiver Report (RR) and measured in seconds. Calculated as
    /// defined in [Section 6.4.1 of RFC 3550][1]. If no RTCP Receiver Report
    /// is received with a DLSR value other than 0, the round trip time is
    /// left undefined.
    ///
    /// [1]: https://tools.ietf.org/html/rfc3550#section-6.4.1
    pub round_trip_time: Option<Float>,

    /// Fraction packet loss reported for this SSRC. Calculated as defined in
    /// [Section 6.4.1 of RFC 3550][1] and [Appendix A.3][2].
    ///
    /// [1]: https://tools.ietf.org/html/rfc3550#section-6.4.1
    /// [2]: https://tools.ietf.org/html/rfc3550#appendix-A.3
    pub fraction_lost: Option<Float>,

    /// Total number of RTCP RR blocks received for this SSRC.
    pub reports_received: Option<u64>,

    /// Total number of RTCP RR blocks received for this SSRC that contain a
    /// valid round trip time. This counter will increment if the
    /// [`roundTripTime`] is undefined.
    ///
    /// [`roundTripTime`]: https://tinyurl.com/ssg83hq
    pub round_trip_time_measurements: Option<Float>,
}

#[cfg(feature = "extended-stats")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-stats")))]
/// [RTCRtpTransceiverStats][1] object representing an [RTCRtpTransceiver] of an
/// [RTCPeerConnection].
///
/// It appears as soon as the monitored [RTCRtpTransceiver] object is created,
/// such as by invoking [addTransceiver][2], [addTrack][3] or
/// [setRemoteDescription][4]. [RTCRtpTransceiverStats][1] objects can only be
/// deleted if the corresponding [RTCRtpTransceiver] is removed (this can only
/// happen if a remote description is rolled back).
///
/// [`RtcStatsType::Transceiver`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
/// [RTCRtpTransceiver]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver
/// [1]: https://w3.org/TR/webrtc-stats/#transceiver-dict%2A
/// [2]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection-addtransceiver
/// [3]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection-addtrack
/// [4]: https://tinyurl.com/vejym8v
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcRtpTransceiverStats {
    /// ID of the stats object representing the
    /// [RTCRtpSender associated with the RTCRtpTransceiver][1] represented by
    /// this stats object.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver-sender
    pub sender_id: Option<String>,

    /// ID of the stats object representing the
    /// [RTCRtpReceiver associated with the RTCRtpTransceiver][1] represented
    /// by this stats object.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver-receiver
    pub receiver_id: Option<String>,

    /// If the [RTCRtpTransceiver] that this stats object represents has a
    /// [`mid` value][1] that is not null, this is that value, otherwise this
    /// value is undefined.
    ///
    /// [RTCRtpTransceiver]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver
    /// [1]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    pub mid: Option<String>,
}

/// Representation of the stats corresponding to an [RTCSctpTransport].
///
/// [`RtcStatsType::SctpTransport`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTCSctpTransport]: https://w3.org/TR/webrtc/#dom-rtcsctptransport
/// [1]: https://w3.org/TR/webrtc-stats/#sctptransportstats-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Copy, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcSctpTransportStats {
    /// Latest smoothed round-trip time value, corresponding to
    /// [`spinfo_srtt` defined in RFC 6458][1] but converted to seconds.
    ///
    /// If there has been no round-trip time measurements yet, this value is
    /// undefined.
    ///
    /// [1]: https://tools.ietf.org/html/rfc6458#page-83
    pub smoothed_round_trip_time: Option<HighResTimeStamp>,
}

/// Representation of the stats corresponding to an [RTCDtlsTransport] and its
/// underlying [RTCIceTransport].
///
/// When RTCP multiplexing is used, one transport is used for both RTP and RTCP.
/// Otherwise, RTP and RTCP will be sent on separate transports, and
/// `rtcpTransportStatsId` can be used to pair the resulting
/// [`RtcTransportStats`] objects. Additionally, when bundling is used, a single
/// transport will be used for all [MediaStreamTrack][2]s in the bundle group.
/// If bundling is not used, different [MediaStreamTrack][2]s will use different
/// transports. RTCP multiplexing and bundling are described in [WebRTC].
///
/// [`RtcStatsType::Transport`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTCDtlsTransport]: https://w3.org/TR/webrtc/#dom-rtcdtlstransport
/// [RTCIceTransport]: https://w3.org/TR/webrtc/#dom-rtcicetransport
/// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
/// [WebRTC]: https://w3.org/TR/webrtc
/// [1]: https://w3.org/TR/webrtc-stats/#transportstats-dict%2A
/// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
#[serde_with::skip_serializing_none]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcTransportStats {
    /// Total number of packets sent over this transport.
    pub packets_sent: Option<u64>,

    /// Total number of packets received on this transport.
    pub packets_received: Option<u64>,

    /// Total number of payload bytes sent on this [RTCPeerConnection], i.e.
    /// not including headers or padding.
    ///
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    pub bytes_sent: Option<u64>,

    /// Total number of bytes received on this [RTCPeerConnection], i.e. not
    /// including headers or padding.
    ///
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    pub bytes_received: Option<u64>,

    /// Set to the current value of the [`role` attribute][1] of the
    /// [underlying RTCDtlsTransport's `transport`][2].
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-icetransport-role
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcdtlstransport-icetransport
    pub ice_role: Option<IceRole>,
}

/// Variants of [ICE roles][1].
///
/// More info in the [RFC 5245].
///
/// [RFC 5245]: https://tools.ietf.org/html/rfc5245
/// [1]: https://w3.org/TR/webrtc/#dom-icetransport-role
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IceRole {
    /// Agent whose role as defined by [Section 3 in RFC 5245][1], has not yet
    /// been determined.
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-3
    Unknown,

    /// Controlling agent as defined by [Section 3 in RFC 5245][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-3
    Controlling,

    /// Controlled agent as defined by [Section 3 in RFC 5245][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-3
    Controlled,
}

#[cfg(feature = "extended-stats")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-stats")))]
/// Statistics related to a specific [RTCRtpSender] and the corresponding
/// media-level metrics.
///
/// [`RtcStatsType::Sender`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTCRtpSender]: https://w3.org/TR/webrtc/#rtcrtpsender-interface
/// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcstatstype-sender
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SenderStatsKind {
    /// [RTCAudioSenderStats][1] object.
    ///
    /// [1]: https://tinyurl.com/w5ow5xs
    Audio { media_source_id: Option<String> },

    /// [RTCVideoSenderStats][1] object.
    ///
    /// [1]: https://tinyurl.com/ry39vnw
    Video { media_source_id: Option<String> },
}

#[cfg(feature = "extended-stats")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-stats")))]
/// Statistics related to a specific [RTCRtpReceiver] and the corresponding
/// media-level metrics.
///
/// [`RtcStatsType::Receiver`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTCRtpReceiver]: https://w3.org/TR/webrtc/#dom-rtcrtpreceiver
/// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcstatstype-receiver
#[serde_with::skip_serializing_none]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReceiverStatsKind {
    /// [RTCAudioReceiverStats] object.
    ///
    /// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcaudioreceiverstats
    Audio {},

    /// [RTCVideoReceiverStats] object.
    ///
    /// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcvideoreceiverstats
    Video {},
}

/// ICE candidate pair statistics related to the [RTCIceTransport] objects.
///
/// A candidate pair that is not the current pair for a transport is
/// [deleted][1] when the [RTCIceTransport] does an ICE restart, at the time
/// the state changes to `new`.
///
/// The candidate pair that is the current pair for a transport is deleted after
/// an ICE restart when the [RTCIceTransport] switches to using a candidate pair
/// generated from the new candidates; this time doesn't correspond to any other
/// externally observable event.
///
/// [`RtcStatsType::CandidatePair`] variant.
///
/// [Full doc on W3C][2].
///
/// [RTCIceTransport]: https://w3.org/TR/webrtc/#dom-rtcicetransport
/// [1]: https://w3.org/TR/webrtc-stats/#dfn-deleted
/// [2]: https://w3.org/TR/webrtc-stats/#candidatepair-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcIceCandidatePairStats {
    /// State of the checklist for the local and remote candidates in a pair.
    pub state: IceCandidatePairState,

    /// Related to updating the nominated flag described in
    /// [Section 7.1.3.2.4 of RFC 5245][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-7.1.3.2.4
    pub nominated: bool,

    /// Total number of payload bytes sent on this candidate pair, i.e. not
    /// including headers or padding.
    pub bytes_sent: u64,

    /// Total number of payload bytes received on this candidate pair, i.e. not
    /// including headers or padding.
    pub bytes_received: u64,

    /// Sum of all round trip time measurements in seconds since the beginning
    /// of the session, based on STUN connectivity check [STUN-PATH-CHAR]
    /// responses (responsesReceived), including those that reply to requests
    /// that are sent in order to verify consent [RFC 7675].
    ///
    /// The average round trip time can be computed from
    /// [`totalRoundTripTime`][1] by dividing it by [`responsesReceived`][2].
    ///
    /// [STUN-PATH-CHAR]: https://w3.org/TR/webrtc-stats/#bib-stun-path-char
    /// [RFC 7675]: https://tools.ietf.org/html/rfc7675
    /// [1]: https://tinyurl.com/tgr543a
    /// [2]: https://tinyurl.com/r3zo2um
    pub total_round_trip_time: Option<HighResTimeStamp>,

    /// Latest round trip time measured in seconds, computed from both STUN
    /// connectivity checks [STUN-PATH-CHAR], including those that are sent for
    /// consent verification [RFC 7675].
    ///
    /// [STUN-PATH-CHAR]: https://w3.org/TR/webrtc-stats/#bib-stun-path-char
    /// [RFC 7675]: https://tools.ietf.org/html/rfc7675
    pub current_round_trip_time: Option<HighResTimeStamp>,

    /// Calculated by the underlying congestion control by combining the
    /// available bitrate for all the outgoing RTP streams using this candidate
    /// pair. The bitrate measurement does not count the size of the IP or
    /// other transport layers like TCP or UDP. It is similar to the TIAS
    /// defined in [RFC 3890], i.e. it is measured in bits per second and the
    /// bitrate is calculated over a 1 second window.
    ///
    /// Implementations that do not calculate a sender-side estimate MUST leave
    /// this undefined. Additionally, the value MUST be undefined for candidate
    /// pairs that were never used. For pairs in use, the estimate is normally
    /// no lower than the bitrate for the packets sent at
    /// [`lastPacketSentTimestamp`][1], but might be higher. For candidate
    /// pairs that are not currently in use but were used before,
    /// implementations MUST return undefined.
    ///
    /// [RFC 3890]: https://tools.ietf.org/html/rfc3890
    /// [1]: https://tinyurl.com/rfc72eh
    pub available_outgoing_bitrate: Option<u64>,
}

/// Each candidate pair in the check list has a foundation and a state.
/// The foundation is the combination of the foundations of the local and
/// remote candidates in the pair.  The state is assigned once the check
/// list for each media stream has been computed.  There are five
/// potential values that the state can have.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownIceCandidatePairState {
    /// Check has not been performed for this pair, and can be performed as
    /// soon as it is the highest-priority Waiting pair on the check list.
    Waiting,

    /// Check has been sent for this pair, but the transaction is in progress.
    InProgress,

    /// Check for this pair was already done and produced a successful result.
    Succeeded,

    /// Check for this pair was already done and failed, either never producing
    /// any response or producing an unrecoverable failure response.
    Failed,

    /// Check for this pair hasn't been performed, and it can't yet be
    /// performed until some other check succeeds, allowing this pair to
    /// unfreeze and move into the [`KnownIceCandidatePairState::Waiting`]
    /// state.
    Frozen,

    /// Other Candidate pair was nominated.
    ///
    /// This state is **obsolete and not spec compliant**, however, it still
    /// may be emitted by some implementations.
    Cancelled,
}

/// Non-exhaustive version of [`KnownIceCandidatePairState`].
pub type IceCandidatePairState = NonExhaustive<KnownIceCandidatePairState>;

/// Known protocols used in the WebRTC.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KnownProtocol {
    /// [User Datagram Protocol][1].
    ///
    /// [1]: https://en.wikipedia.org/wiki/User_Datagram_Protocol
    Udp,

    /// [Transmission Control Protocol][1].
    ///
    /// [1]: https://en.wikipedia.org/wiki/Transmission_Control_Protocol
    Tcp,
}

/// Non-exhaustive version of [`KnownProtocol`].
pub type Protocol = NonExhaustive<KnownProtocol>;

/// [RTCIceCandidateType] represents the type of the ICE candidate, as
/// defined in [Section 15.1 of RFC 5245][1].
///
/// [RTCIceCandidateType]: https://w3.org/TR/webrtc/#rtcicecandidatetype-enum
/// [1]: https://tools.ietf.org/html/rfc5245#section-15.1
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KnownCandidateType {
    /// Host candidate, as defined in [Section 4.1.1.1 of RFC 5245][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-4.1.1.1
    Host,

    /// Server reflexive candidate, as defined in
    /// [Section 4.1.1.2 of RFC 5245][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-4.1.1.2
    Srlfx,

    /// Peer reflexive candidate, as defined in
    /// [Section 4.1.1.2 of RFC 5245][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-4.1.1.2
    Prflx,

    /// Relay candidate, as defined in [Section 7.1.3.2.1 of RFC 5245][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-7.1.3.2.1
    Relay,
}

/// Non-exhaustive version of [`KnownCandidateType`].
pub type CandidateType = NonExhaustive<KnownCandidateType>;

/// Fields of [`RtcStatsType::InboundRtp`] variant.
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(tag = "mediaType", rename_all = "camelCase")]
pub enum RtcInboundRtpStreamMediaType {
    /// Fields when `mediaType` is `audio`.
    Audio {
        /// Indicator whether the last RTP packet whose frame was delivered to
        /// the [RTCRtpReceiver]'s [MediaStreamTrack][1] for playout contained
        /// voice activity or not based on the presence of the V bit in the
        /// extension header, as defined in [RFC 6464].
        ///
        /// [RTCRtpReceiver]: https://w3.org/TR/webrtc/#rtcrtpreceiver-interface
        /// [RFC 6464]: https://tools.ietf.org/html/rfc6464#page-3
        /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
        voice_activity_flag: Option<bool>,

        /// Total number of samples that have been received on this RTP stream.
        /// This includes [`concealedSamples`].
        ///
        /// [`concealedSamples`]: https://tinyurl.com/s6c4qe4
        total_samples_received: Option<u64>,

        /// Total number of samples that are concealed samples.
        ///
        /// A concealed sample is a sample that was replaced with synthesized
        /// samples generated locally before being played out.
        /// Examples of samples that have to be concealed are samples from lost
        /// packets (reported in [`packetsLost`]) or samples from packets that
        /// arrive too late to be played out (reported in
        /// [`packetsDiscarded`]).
        ///
        /// [`packetsLost`]: https://tinyurl.com/u2gq965
        /// [`packetsDiscarded`]: https://tinyurl.com/yx7qyox3
        concealed_samples: Option<u64>,

        /// Total number of concealed samples inserted that are "silent".
        ///
        /// Playing out silent samples results in silence or comfort noise.
        /// This is a subset of [`concealedSamples`].
        ///
        /// [`concealedSamples`]: https://tinyurl.com/s6c4qe4
        silent_concealed_samples: Option<u64>,

        /// Audio level of the receiving track.
        audio_level: Option<Float>,

        /// Audio energy of the receiving track.
        total_audio_energy: Option<Float>,

        /// Audio duration of the receiving track.
        ///
        /// For audio durations of tracks attached locally, see
        /// [RTCAudioSourceStats][1] instead.
        ///
        /// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcaudiosourcestats
        total_samples_duration: Option<HighResTimeStamp>,
    },

    /// Fields when `mediaType` is `video`.
    Video {
        /// Total number of frames correctly decoded for this RTP stream, i.e.
        /// frames that would be displayed if no frames are dropped.
        frames_decoded: Option<u64>,

        /// Total number of key frames, such as key frames in VP8 [RFC 6386] or
        /// IDR-frames in H.264 [RFC 6184], successfully decoded for this RTP
        /// media stream.
        ///
        /// This is a subset of [`framesDecoded`].
        /// [`framesDecoded`] - [`keyFramesDecoded`] gives you the number of
        /// delta frames decoded.
        ///
        /// [RFC 6386]: https://w3.org/TR/webrtc-stats/#bib-rfc6386
        /// [RFC 6184]: https://w3.org/TR/webrtc-stats/#bib-rfc6184
        /// [`framesDecoded`]: https://tinyurl.com/srfwrwt
        /// [`keyFramesDecoded`]: https://tinyurl.com/qtdmhtm
        key_frames_decoded: Option<u64>,

        /// Width of the last decoded frame.
        ///
        /// Before the first frame is decoded this attribute is missing.
        frame_width: Option<u64>,

        /// Height of the last decoded frame.
        ///
        /// Before the first frame is decoded this attribute is missing.
        frame_height: Option<u64>,

        /// Sum of the interframe delays in seconds between consecutively
        /// decoded frames, recorded just after a frame has been decoded.
        total_inter_frame_delay: Option<Float>,

        /// Number of decoded frames in the last second.
        frames_per_second: Option<u64>,

        /// Bit depth per pixel of the last decoded frame.
        ///
        /// Typical values are 24, 30, or 36 bits. Before the first frame is
        /// decoded this attribute is missing.
        frame_bit_depth: Option<u64>,

        /// Total number of Full Intra Request (FIR) packets sent by this
        /// receiver.
        fir_count: Option<u64>,

        /// Total number of Picture Loss Indication (PLI) packets sent by this
        /// receiver.
        pli_count: Option<u64>,

        /// Total number of Slice Loss Indication (SLI) packets sent by this
        /// receiver.
        sli_count: Option<u64>,

        /// Number of concealment events.
        ///
        /// This counter increases every time a concealed sample is synthesized
        /// after a non-concealed sample. That is, multiple consecutive
        /// concealed samples will increase the [`concealedSamples`] count
        /// multiple times but is a single concealment event.
        ///
        /// [`concealedSamples`]: https://tinyurl.com/s6c4qe4
        concealment_events: Option<u64>,

        /// Total number of complete frames received on this RTP stream.
        ///
        /// This metric is incremented when the complete frame is received.
        frames_received: Option<u64>,
    },
}

/// Representation of the measurement metrics for the incoming [RTP] media
/// stream. The timestamp reported in the statistics object is the time at which
/// the data was sampled.
///
/// [`RtcStatsType::InboundRtp`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
/// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcInboundRtpStreamStats {
    /// ID of the stats object representing the receiving track.
    pub track_id: Option<String>,

    /// Fields which should be in the [`RtcStat`] based on `mediaType`.
    #[serde(flatten)]
    pub media_specific_stats: RtcInboundRtpStreamMediaType,

    /// Total number of bytes received for this SSRC.
    pub bytes_received: u64,

    /// Total number of RTP data packets received for this SSRC.
    pub packets_received: u64,

    /// Total number of RTP data packets for this SSRC that have been lost
    /// since the beginning of reception.
    ///
    /// This number is defined to be the number of packets expected less the
    /// number of packets actually received, where the number of packets
    /// received includes any which are late or duplicates. Thus, packets that
    /// arrive late are not counted as lost, and the loss __may be negative__
    /// if there are duplicates.
    pub packets_lost: Option<i64>,

    /// Packet jitter measured in seconds for this SSRC.
    pub jitter: Option<Float>,

    /// Total number of seconds that have been spent decoding the
    /// [`framesDecoded`] frames of this stream.
    ///
    /// The average decode time can be calculated by dividing this value with
    /// [`framesDecoded`]. The time it takes to decode one frame is the time
    /// passed between feeding the decoder a frame and the decoder returning
    /// decoded data for that frame.
    ///
    /// [`framesDecoded`]: https://tinyurl.com/srfwrwt
    pub total_decode_time: Option<HighResTimeStamp>,

    /// Total number of audio samples or video frames that have come out of the
    /// jitter buffer (increasing [`jitterBufferDelay`]).
    ///
    /// [`jitterBufferDelay`]: https://tinyurl.com/qvoojt5
    pub jitter_buffer_emitted_count: Option<u64>,
}

/// Statistics related to a specific [MediaStreamTrack][1]'s attachment to an
/// [RTCRtpSender] and the corresponding media-level metrics.
///
/// [`RtcStatsType::Track`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTCRtpSender]: https://w3.org/TR/webrtc/#rtcrtpsender-interface
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
/// [2]: https://w3.org/TR/webrtc-stats/#dom-rtcstatstype-track
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackStats {
    /// [`id` property][1] of the track.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    pub track_identifier: String,

    /// `true` if the source is remote, for instance if it is sourced from
    /// another host via an [RTCPeerConnection]. `false` otherwise.
    ///
    /// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
    pub remote_source: Option<bool>,

    /// Reflection of the "ended" state of the track.
    pub ended: Option<bool>,

    /// Either `audio` or `video`.
    ///
    /// This reflects the [`kind` attribute][2] of the [MediaStreamTrack][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    /// [2]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-kind
    pub kind: Option<TrackStatsKind>,
}

/// [`kind` attribute] values of the [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
/// [2]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-kind
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackStatsKind {
    /// Track is used for the audio content.
    Audio,

    /// Track is used for the video content.
    Video,
}

/// [`RtcStat`] fields of [`RtcStatsType::OutboundRtp`] type based on
/// `mediaType`.
#[serde_with::skip_serializing_none]
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "mediaType", rename_all = "camelCase")]
pub enum RtcOutboundRtpStreamMediaType {
    /// Fields when `mediaType` is `audio`.
    Audio {
        /// Total number of samples that have been sent over this RTP stream.
        total_samples_sent: Option<u64>,

        /// Whether the last RTP packet sent contained voice activity or not
        /// based on the presence of the V bit in the extension header.
        voice_activity_flag: Option<bool>,
    },

    /// Fields when `mediaType` is `video`.
    Video {
        /// Width of the last encoded frame.
        ///
        /// The resolution of the encoded frame may be lower than the media
        /// source (see [RTCVideoSourceStats.width][1]).
        ///
        /// Before the first frame is encoded this attribute is missing.
        ///
        /// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcvideosourcestats-width
        frame_width: Option<u64>,

        /// Height of the last encoded frame.
        ///
        /// The resolution of the encoded frame may be lower than the media
        /// source (see [RTCVideoSourceStats.height][1]).
        ///
        /// Before the first frame is encoded this attribute is missing.
        ///
        /// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcvideosourcestats-height
        frame_height: Option<u64>,

        /// Number of encoded frames during the last second.
        ///
        /// This may be lower than the media source frame rate (see
        /// [RTCVideoSourceStats.framesPerSecond][1]).
        ///
        /// [1]: https://tinyurl.com/rrmkrfk
        frames_per_second: Option<u64>,
    },
}

/// Statistics for an outbound [RTP] stream that is currently sent with this
/// [RTCPeerConnection] object.
///
/// When there are multiple [RTP] streams connected to the same sender, such
/// as when using simulcast or RTX, there will be one
/// [`RtcOutboundRtpStreamStats`] per RTP stream, with distinct values of
/// the `ssrc` attribute, and all these senders will have a reference to
/// the same "sender" object (of type [RTCAudioSenderStats][1] or
/// [RTCVideoSenderStats][2]) and "track" object (of type
/// [RTCSenderAudioTrackAttachmentStats][3] or
/// [RTCSenderVideoTrackAttachmentStats][4]).
///
/// [`RtcStatsType::OutboundRtp`] variant.
///
/// [Full doc on W3C][5].
///
/// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
/// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
/// [1]: https://w3.org/TR/webrtc-stats/#dom-rtcaudiosenderstats
/// [2]: https://w3.org/TR/webrtc-stats/#dom-rtcvideosenderstats
/// [3]: https://tinyurl.com/sefa5z4
/// [4]: https://tinyurl.com/rkuvpl4
/// [5]: https://w3.org/TR/webrtc-stats/#outboundrtpstats-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcOutboundRtpStreamStats {
    /// ID of the stats object representing the current track attachment to the
    /// sender of this stream.
    pub track_id: Option<String>,

    /// Fields which should be in the [`RtcStat`] based on `mediaType`.
    #[serde(flatten)]
    pub media_type: RtcOutboundRtpStreamMediaType,

    /// Total number of bytes sent for this SSRC.
    pub bytes_sent: u64,

    /// Total number of RTP packets sent for this SSRC.
    pub packets_sent: u64,

    /// ID of the stats object representing the track currently
    /// attached to the sender of this stream.
    pub media_source_id: Option<String>,
}

/// Properties of a `candidate` in [Section 15.1 of RFC 5245][1].
/// It corresponds to a [RTCIceTransport] object.
///
/// [`RtcStatsType::LocalCandidate`] or [`RtcStatsType::RemoteCandidate`]
/// variant.
///
/// [Full doc on W3C][2].
///
/// [RTCIceTransport]: https://w3.org/TR/webrtc/#dom-rtcicetransport
/// [1]: https://tools.ietf.org/html/rfc5245#section-15.1
/// [2]: https://w3.org/TR/webrtc-stats/#icecandidate-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcIceCandidateStats {
    /// Unique ID that is associated to the object that was inspected to
    /// produce the [RTCTransportStats][1] associated with this candidate.
    ///
    /// [1]: https://w3.org/TR/webrtc-stats/#transportstats-dict%2A
    pub transport_id: Option<String>,

    /// Address of the candidate, allowing for IPv4 addresses, IPv6 addresses,
    /// and fully qualified domain names (FQDNs).
    pub address: Option<String>,

    /// Port number of the candidate.
    pub port: u16,

    /// Valid values for transport is one of `udp` and `tcp`.
    pub protocol: Protocol,

    /// Type of the ICE candidate.
    pub candidate_type: CandidateType,

    /// Calculated as defined in [Section 15.1 of RFC 5245][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc5245#section-15.1
    pub priority: u32,

    /// For local candidates this is the URL of the ICE server from which the
    /// candidate was obtained. It is the same as the
    /// [url surfaced in the RTCPeerConnectionIceEvent][1].
    ///
    /// `None` for remote candidates.
    ///
    /// [1]: https://w3.org/TR/webrtc/#rtcpeerconnectioniceevent
    pub url: Option<String>,

    /// Protocol used by the endpoint to communicate with the TURN server.
    ///
    /// Only present for local candidates.
    pub relay_protocol: Option<Protocol>,
}

/// [`RtcStat`] fields of [`RtcStatsType::MediaSource`] type based on its
/// `kind`.
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MediaKind {
    /// Fields when `kind` is `video`.
    Video {
        /// Width (in pixels) of the last frame originating from the source.
        /// Before a frame has been produced this attribute is missing.
        width: Option<u32>,

        /// Height (in pixels) of the last frame originating from the source.
        /// Before a frame has been produced this attribute is missing.
        height: Option<u32>,

        /// Number of frames originating from the source, measured during the
        /// last second. For the first second of this object's lifetime this
        /// attribute is missing.
        frames_per_second: Option<u32>,
    },

    /// Fields when `kind` is `audio`.
    Audio {
        /// Audio level of the media source.
        audio_level: Option<Float>,

        /// Audio energy of the media source.
        total_audio_energy: Option<Float>,

        /// Audio duration of the media source.
        total_samples_duration: Option<Float>,
    },
}

/// Statistics for the media produced by a [MediaStreamTrack][1] that is
/// currently attached to an [RTCRtpSender]. This reflects the media that is fed
/// to the encoder after [getUserMedia] constraints have been applied (i.e. not
/// the raw media produced by the camera).
///
/// [`RtcStatsType::MediaSource`] variant.
///
/// [Full doc on W3C][2].
///
/// [RTCRtpSender]: https://w3.org/TR/webrtc/#rtcrtpsender-interface
/// [getUserMedia]: https://tinyurl.com/sngpyr6
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
/// [2]: https://w3.org/TR/webrtc-stats/#dom-rtcstatstype-media-source
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSourceStats {
    /// Value of the [MediaStreamTrack][1]'s ID attribute.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub track_identifier: Option<String>,

    /// Fields which should be in the [`RtcStat`] based on `kind`.
    #[serde(flatten)]
    pub kind: MediaKind,
}

#[cfg(feature = "extended-stats")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-stats")))]
/// Statistics for a codec that is currently used by [RTP] streams being sent or
/// received by [RTCPeerConnection] object.
///
/// [`RtcStatsType::Codec`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
/// [RTCPeerConnection]: https://w3.org/TR/webrtc/#dom-rtcpeerconnection
/// [1]: https://w3.org/TR/webrtc-stats/#dom-rtccodecstats
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcCodecStats {
    /// [Payload type][1] as used in [RTP] encoding or decoding.
    ///
    /// [RTP]: https://en.wikipedia.org/wiki/Real-time_Transport_Protocol
    /// [1]: https://tools.ietf.org/html/rfc3550#page-14
    pub payload_type: u32,

    /// The codec MIME media `type/subtype` (e.g. `video/vp8` or equivalent).
    pub mime_type: String,

    /// Media sampling rate.
    pub clock_rate: u32,
}

#[cfg(feature = "extended-stats")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-stats")))]
/// Information about a certificate used by [RTCIceTransport].
///
/// [`RtcStatsType::Certificate`] variant.
///
/// [Full doc on W3C][1].
///
/// [RTCIceTransport]: https://w3.org/TR/webrtc/#dom-rtcicetransport
/// [1]: https://w3.org/TR/webrtc-stats/#certificatestats-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcCertificateStats {
    /// Fingerprint of the certificate.
    ///
    /// Only use the fingerprint value as defined in [Section 5 of RFC
    /// 4572][1].
    ///
    /// [1]: https://tools.ietf.org/html/rfc4572#section-5
    pub fingerprint: String,

    /// Hash function used to compute the certificate fingerprint.
    /// For instance, `sha-256`.
    pub fingerprint_algorithm: String,

    /// The DER-encoded Base64 representation of the certificate.
    pub base64_certificate: String,
}

/// Representation of [DOMHighResTimeStamp][1].
///
/// Can be converted to the [`SystemTime`] with millisecond-wise accuracy.
///
/// [`HighResTimeStamp`] type is a [`f64`] and is used to store a time value
/// in milliseconds. This type can be used to describe a discrete point in time
/// or a time interval (the difference in time between two discrete points in
/// time).
///
/// The time, given in milliseconds, should be accurate to 5 µs (microseconds),
/// with the fractional part of the number indicating fractions of a
/// millisecond. However, if the browser is unable to provide a time value
/// accurate to 5 µs (due, for example, to hardware or software constraints),
/// the browser can represent the value as a time in milliseconds accurate to a
/// millisecond. Also note the section below on reduced time precision
/// controlled by browser preferences to avoid timing attacks and
/// fingerprinting.
///
/// Further, if the device or operating system the user agent is running on
/// doesn't have a clock accurate to the microsecond level, they may only be
/// accurate to the millisecond.
///
/// [1]: https://developer.mozilla.org/docs/Web/API/DOMHighResTimeStamp
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct HighResTimeStamp(pub f64);

impl From<HighResTimeStamp> for SystemTime {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[inline]
    fn from(timestamp: HighResTimeStamp) -> Self {
        SystemTime::UNIX_EPOCH + Duration::from_millis(timestamp.0 as u64)
    }
}

impl From<SystemTime> for HighResTimeStamp {
    #[allow(clippy::cast_precision_loss)]
    #[inline]
    fn from(time: SystemTime) -> Self {
        HighResTimeStamp(
            time.duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as f64,
        )
    }
}

/// Hashing string representation.
///
/// Some people believe that such behavior is incorrect (but in some programming
/// languages this is a default behavior) due to `NaN`, `Inf` or `-Inf` (they
/// all will have the same hashes).
/// But in the case of [`RtcStat`] received from the client, there should be no
/// such situations, and the hash will always be correct.
impl Hash for HighResTimeStamp {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_string().hash(state);
    }
}

/// Comparison string representations.
///
/// Such implementation is required, so that the results of comparing values and
/// comparing hashes match.
impl PartialEq for HighResTimeStamp {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string().eq(&other.0.to_string())
    }
}

/// [`f64`] wrapper with [`Hash`] implementation.
#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub struct Float(pub f64);

/// Hashing string representation.
///
/// Some people believe that such behavior is incorrect (but in some programming
/// languages this is a default behavior) due to `NaN`, `Inf` or `-Inf` (they
/// all will have the same hashes).
/// But in the case of [`RtcStat`] received from the client, there should be no
/// such situations, and the hash will always be correct.
impl Hash for Float {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_string().hash(state);
    }
}

/// Comparison string representations.
///
/// Such implementation is required, so that the results of comparing values and
/// comparing hashes match.
impl PartialEq for Float {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string().eq(&other.0.to_string())
    }
}

#[cfg(feature = "extended-stats")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-stats")))]
/// Information about the connection to an ICE server (e.g. STUN or TURN).
///
/// [`RtcStatsType::IceServer`] variant.
///
/// [Full doc on W3C][1].
///
/// [1]: https://w3.org/TR/webrtc-stats/#ice-server-dict%2A
#[serde_with::skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcIceServerStats {
    /// URL of the ICE server (e.g. TURN or STUN server).
    pub url: String,

    /// Port number used by the client.
    pub port: u16,

    /// Protocol used by the client to connect to ICE server.
    pub protocol: Protocol,

    /// Total amount of requests that have been sent to this server.
    pub total_requests_sent: Option<u64>,

    /// Total amount of responses received from this server.
    pub total_responses_received: Option<u64>,

    /// Sum of RTTs for all requests that have been sent where a response has
    /// been received.
    pub total_round_trip_time: Option<HighResTimeStamp>,
}
