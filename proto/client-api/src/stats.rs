//! Contains DTO's for [RTCPeerConnection][1] metrics.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcpeerconnection-interface

// TODO: changelog, rename `unused` feature to something more meaningful.

#![allow(clippy::module_name_repetitions)]

use std::{
    hash::{Hash, Hasher},
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};

/// Enum with which you can try to deserialize some known enum and if it
/// isn't known, then unknown data will be stored as [`String`] in the
/// [`NonExhaustive::Unknown`] variant.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(untagged)]
pub enum NonExhaustive<T> {
    /// Will store known enum variant if it successfully deserialized.
    Known(T),

    /// Will store unknown enum variant with it's data as [`String`].
    Unknown(String),
}

/// A unique id that is associated with the object that was inspected to produce
/// [`RtcStat`] object.
///
/// Two [`RtcStat`]s objects, extracted from two different [`RTCStatsReport`]
/// objects, MUST have the same ID if they were produced by inspecting the same
/// underlying object.
///
/// [`RTCStatsReport`]: https://www.w3.org/TR/webrtc/#dom-rtcstatsreport
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
pub struct StatId(pub String);

/// Represents the [stats object] constructed by inspecting a specific
/// [monitored object].
///
/// [Full doc on W3C][1]
///
/// [stats object]: https://www.w3.org/TR/webrtc-stats/#dfn-stats-object
/// [monitored object]: https://www.w3.org/TR/webrtc-stats/#dfn-monitored-object
/// [1]: https://www.w3.org/TR/webrtc/#rtcstats-dictionary
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
pub struct RtcStat<T> {
    /// A unique id that is associated with the object that was inspected to
    /// produce this RTCStats object.
    pub id: StatId,

    /// The timestamp, of type [`HighResTimeStamp`], associated with
    /// this object. The time is relative to the UNIX epoch (Jan 1, 1970, UTC).
    /// For statistics that came from a remote source (e.g., from received RTCP
    /// packets), timestamp represents the time at which the information
    /// arrived at the local endpoint. The remote timestamp can be found in an
    /// additional field in an [`RtcStat`]-derived dictionary, if applicable.
    pub timestamp: HighResTimeStamp,

    /// Actual stats of this [`RtcStat`].
    ///
    /// All possible stats are described in the [`RtcStatsType`] enum.
    #[serde(flatten)]
    pub stats: Box<T>,
}

/// Represents all known types of [`RtcStat`]s.
///
/// [List of all `RtcStat` types on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#rtctatstype-*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
pub enum RtcStatsType {
    /// Statistics for a codec that is currently being used by RTP streams
    /// being sent or received by this `RTCPeerConnection` object. It is
    /// accessed by the [`RtcCodecStats`].
    #[cfg(feature = "unused")]
    Codec(RtcStat<RtcCodecStats>),

    /// Statistics for an inbound RTP stream that is currently received with
    /// this RTCPeerConnection object. It is accessed by the
    /// [`RtcInboundRtpStreamStats`].
    InboundRtp(RtcStat<RtcInboundRtpStreamStats>),

    /// Statistics for an outbound RTP stream that is currently sent with this
    /// `RTCPeerConnection` object. It is accessed by the
    /// [`RtcOutboundRtpStreamStats`].
    ///
    /// When there are multiple RTP streams connected to the same sender, such
    /// as when using simulcast or RTX, there will be one
    /// [`RtcOutboundRtpStreamStats`] per RTP stream, with distinct values of
    /// the "ssrc" attribute, and all these senders will have a reference to
    /// the same "sender" object (of type `RTCAudioSenderStats` or
    /// `RTCVideoSenderStats`) and "track" object (of type
    /// `RTCSenderAudioTrackAttachmentStats` or
    /// `RTCSenderVideoTrackAttachmentStats`).
    OutboundRtp(RtcStat<RtcOutboundRtpStreamStats>),

    /// Statistics for the remote endpoint's inbound RTP stream corresponding
    /// to an outbound stream that is currently sent with this
    /// `RTCPeerConnection` object. It is measured at the remote endpoint and
    /// reported in an RTCP Receiver Report (RR) or RTCP Extended Report (XR).
    RemoteInboundRtp(RtcStat<RemoteInboundRtpStreamStat>),

    /// Statistics for the remote endpoint's outbound RTP stream corresponding
    /// to an inbound stream that is currently received with this
    /// `RTCPeerConnection` object. It is measured at the remote endpoint and
    /// reported in an RTCP Sender Report (SR).
    RemoteOutboundRtp(RtcStat<RemoteOutboundRtpStreamStat>),

    /// Statistics for the media produced by a `MediaStreamTrack`that is
    /// currently attached to an `RTCRtpSender`. This reflects the media that
    /// is fed to the encoder; after `getUserMedia()` constraints have been
    /// applied (i.e. not the raw media produced by the camera).
    MediaSource(RtcStat<MediaSourceStat>),

    /// Statistics for a contributing source (CSRC) that contributed to an
    /// inbound RTP stream.
    #[cfg(feature = "unused")]
    Csrc(RtcStat<RtpContributingSourceStat>),

    /// Statistics related to the `RTCPeerConnection` object.
    #[cfg(feature = "unused")]
    PeerConnection(RtcStat<RtcPeerConnectionStat>),

    /// Statistics related to each RTCDataChannel id.
    #[cfg(feature = "unused")]
    DataChannel(RtcStat<DataChannelStat>),

    /// This is now obsolete. Contains statistics related to a specific
    /// MediaStream.
    #[cfg(feature = "unused")]
    Stream(RtcStat<MediaStreamStat>),

    /// Statistics related to a specific MediaStreamTrack's attachment to an
    /// RTCRtpSender and the corresponding media-level metrics.
    // maybe
    Track(RtcStat<TrackStat>),

    /// Statistics related to a specific `RTCRtpTransceiver`.
    #[cfg(feature = "unused")]
    Transceiver(RtcStat<RtcRtpTransceiverStats>),

    /// Statistics related to a specific `RTCRtpSender` and the corresponding
    /// media-level metrics.
    #[cfg(feature = "unused")]
    Sender(RtcStat<SenderStatsKind>),

    /// Statistics related to a specific receiver and the corresponding
    /// media-level metrics.
    #[cfg(feature = "unused")]
    Receiver(RtcStat<ReceiverStatsKind>),

    /// Transport statistics related to the `RTCPeerConnection` object.
    Transport(RtcStat<RtcTransportStats>),

    /// SCTP transport statistics related to an `RTCSctpTransport` object.
    // maybe
    SctpTransport(RtcStat<RtcSctpTransportStats>),

    /// ICE candidate pair statistics related to the `RTCIceTransport` objects.
    ///
    /// A candidate pair that is not the current pair for a transport is
    /// [deleted] when the `RTCIceTransport` does an ICE restart, at the time
    /// the state changes to "new". The candidate pair that is the current
    /// pair for a transport is deleted after an ICE restart when the
    /// `RTCIceTransport` switches to using a candidate pair generated from
    /// the new candidates; this time doesn't correspond to any other
    /// externally observable event.
    ///
    /// [deleted]: https://www.w3.org/TR/webrtc-stats/#dfn-deleted
    CandidatePair(RtcStat<RtcIceCandidatePairStats>),

    /// ICE local candidate statistics related to the `RTCIceTransport`
    /// objects.
    ///
    /// A local candidate is [deleted] when the `RTCIceTransport` does an ICE
    /// restart, and the candidate is no longer a member of any non-deleted
    /// candidate pair.
    ///
    /// [deleted]: https://www.w3.org/TR/webrtc-stats/#dfn-deleted
    LocalCandidate(RtcStat<RtcIceCandidateStats>),

    /// ICE remote candidate statistics related to the `RTCIceTransport`
    /// objects.
    ///
    /// A remote candidate is [deleted] when the `RTCIceTransport` does an ICE
    /// restart, and the candidate is no longer a member of any non-deleted
    /// candidate pair.
    ///
    /// [deleted]: https://www.w3.org/TR/webrtc-stats/#dfn-deleted
    RemoteCandidate(RtcStat<RtcIceCandidateStats>),

    /// Information about a certificate used by an `RTCIceTransport`.
    #[cfg(feature = "unused")]
    Certificate(RtcStat<RtcCertificateStats>),

    /// Information about the connection to an ICE server (e.g. STUN or TURN).
    #[cfg(feature = "unused")]
    IceServer(RtcStat<RtcIceServerStats>),

    /// Disabled or unknown variants of stats will be deserialized as
    /// [`RtcStatsType::Other`].
    #[serde(other)]
    Other,
}

/// This is now obsolete. Contains statistics related to a specific
/// `MediaStream`.
///
/// [`RtcStatsType::Stream`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#idl-def-rtcmediastreamstats
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub struct MediaStreamStat {
    /// `stream.id` property.
    #[serde(rename = "streamIdentifier")]
    pub stream_id: String,

    /// This is the id of the stats object, not the `track.id`.
    pub track_ids: Vec<String>,
}

/// Statistics related to each RTCDataChannel id.
///
/// [`RtcStatsType::DataChannel`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#dcstats-dict*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub struct DataChannelStat {
    /// The "label" value of the [`RTCDataChannel`] object.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    pub label: Option<String>,

    /// The "protocol" value of the [`RTCDataChannel`] object.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    pub protocol: Option<Protocol>,

    /// The "id" attribute of the [`RTCDataChannel`] object.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    #[serde(rename = "dataChannelIdentifier")]
    pub data_channel_id: Option<u64>,

    /// A [stats object reference][1] for the transport used to carry this
    /// datachannel.
    ///
    /// [1]: https://www.w3.org/TR/webrtc-stats/#dfn-stats-object-reference
    pub transport_id: Option<String>,

    /// The "readyState" value of the [`RTCDataChannel`] object.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    pub state: Option<DataChannelState>,

    /// Represents the total number of API "message" events sent.
    pub messages_sent: Option<u64>,

    /// Represents the total number of payload bytes sent on this
    /// [`RTCDataChannel`], i.e., not including headers or padding.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    pub bytes_sent: Option<u64>,

    /// Represents the total number of API "message" events received.
    pub messages_received: Option<u64>,

    /// Represents the total number of bytes received on this
    /// [`RTCDataChannel`], i.e., not including headers or padding.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    pub bytes_received: Option<u64>,
}

/// Stores known [`KnownDataChannelState`] (as [`NonExhaustive::Known`] enum
/// variant) or unknown state (as [`NonExhaustive::Unknown`] enum variant).
pub type DataChannelState = NonExhaustive<KnownDataChannelState>;

/// Indicates the state of the data channel's underlying data connection.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum KnownDataChannelState {
    /// The user agent is attempting to establish the underlying data
    /// transport. This is the initial state of an [`RTCDataChannel`]
    /// object, whether created with [`createDataChannel`], or dispatched
    /// as a part of an [`RTCDataChannelEvent`].
    ///
    /// [`RTCDataChannel`]: https://www.w3.org/TR/webrtc/#dom-rtcdatachannel
    /// [`createDataChannel`]:
    /// https://www.w3.org/TR/webrtc/#dom-peerconnection-createdatachannel
    /// [`RTCDataChannelEvent`]:
    /// https://www.w3.org/TR/webrtc/#dom-rtcdatachannelevent
    Connecting,

    /// The [underlying data transport][1] is established and communication is
    /// possible.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dfn-data-transport
    Open,

    /// The [`procedure`] to close down the [underlying data transport][1] has
    /// started.
    ///
    /// [`procedure`]:
    /// https://www.w3.org/TR/webrtc/#data-transport-closing-procedure
    /// [1]: https://www.w3.org/TR/webrtc/#dfn-data-transport
    Closing,

    /// The [underlying data transport][1] has been [`closed`] or could not be
    /// established.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dfn-data-transport
    /// [`closed`]: https://www.w3.org/TR/webrtc/#dom-rtcdatachannelstate-closed
    Closed,
}

/// Stats for the [`RTCPeerConnection`] object.
///
/// [`RtcStatsType::PeerConnection`] variant.
///
/// [Full doc on W3C][1]
///
/// [`RTCPeerConnection`]:
/// https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection
/// [1]: https://www.w3.org/TR/webrtc-stats/#pcstats-dict*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub struct RtcPeerConnectionStat {
    /// Represents the number of unique `DataChannel`s that have entered the
    /// "open" state during their lifetime.
    pub data_channels_opened: Option<u64>,

    /// Represents the number of unique `DataChannel`s that have left the
    /// "open" state during their lifetime (due to being closed by either
    /// end or the underlying transport being closed). `DataChannel`s that
    /// transition from "connecting" to "closing" or "closed" without ever
    /// being "open" are not counted in this number.
    pub data_channels_closed: Option<u64>,

    /// Represents the number of unique `DataChannel`s returned from a
    /// successful `createDataChannel()` call on the `RTCPeerConnection`.
    /// If the underlying data transport is not established, these may be
    /// in the "connecting" state.
    pub data_channels_requested: Option<u64>,

    /// Represents the number of unique `DataChannel`s signaled in a
    /// "datachannel" event on the `RTCPeerConnection`.
    pub data_channels_accepted: Option<u64>,
}

/// Statistics for a contributing source (CSRC) that contributed to an
/// inbound RTP stream.
///
/// [`RtcStatsType::Csrc`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#contributingsourcestats-dict*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub struct RtpContributingSourceStat {
    /// The SSRC identifier of the contributing source represented by this
    /// stats object, as defined by [RFC3550]. It is a 32-bit unsigned integer
    /// that appears in the CSRC list of any packets the relevant source
    /// contributed to.
    ///
    /// [RFC3550]: https://www.w3.org/TR/webrtc-stats/#bib-rfc3550
    pub contributor_ssrc: Option<u32>,

    /// The ID of the [`RTCInboundRtpStreamStats`] object representing the
    /// inbound RTP stream that this contributing source is contributing to.
    ///
    /// [`RTCInboundRtpStreamStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
    pub inbound_rtp_stream_id: Option<String>,

    /// The total number of RTP packets that this contributing source
    /// contributed to. This value is incremented each time a packet is counted
    /// by [`RTCInboundRtpStreamStats.packetsReceived`], and the packet's CSRC
    /// list (as defined by [RFC3550] section 5.1) contains the SSRC identifier
    /// of this contributing source, [`contributorSsrc`].
    ///
    /// [`RTCInboundRtpStreamStats.packetsReceived`]:
    /// https://tinyurl.com/rreuf49
    /// [`contributorSsrc`]: https://tinyurl.com/tf8c7j4
    pub packets_contributed_to: Option<u64>,

    /// Present if the last received RTP packet that this source contributed to
    /// contained an [RFC6465] mixer-to-client audio level header extension.
    /// The value of `audioLevel` is between `0..1` (linear), where `1.0`
    /// represents `0 dBov`, `0` represents silence, and `0.5` represents
    /// approximately `6 dBSPL` change in the sound pressure level from 0
    /// dBov. The [RFC6465] header extension contains values in the range
    /// `0..127`, in units of `-dBov`, where `127` represents silence. To
    /// convert these values to the linear `0..1` range of `audioLevel`, a
    /// value of `127` is converted to `0`, and all other values are
    /// converted using the equation: `f(rfc6465_level) =
    /// 10^(-rfc6465_level/20)`.
    ///
    /// [RFC6465]: https://www.w3.org/TR/webrtc-stats/#bib-rfc6465
    pub audio_level: Option<Float>,
}

/// Statistics for the remote endpoint's outbound RTP stream corresponding
/// to an inbound stream that is currently received with this
/// `RTCPeerConnection` object. It is measured at the remote endpoint and
/// reported in an RTCP Sender Report (SR).
///
/// [`RtcStatsType::RemoteOutbountRtp`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#remoteoutboundrtpstats-dict*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct RemoteOutboundRtpStreamStat {
    /// The `localId` is used for looking up the local
    /// [`RTCInboundRtpStreamStats`] object for the same SSRC.
    ///
    /// [`RTCInboundRtpStreamStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
    pub local_id: Option<String>,

    /// `remoteTimestamp`, of type `DOMHighResTimeStamp` [HIGHRES-TIME],
    /// represents the remote timestamp at which these statistics were sent by
    /// the remote endpoint. This differs from timestamp, which represents the
    /// time at which the statistics were generated or received by the local
    /// endpoint. The remoteTimestamp, if present, is derived from the NTP
    /// timestamp in an RTCP Sender Report (SR) block, which reflects the
    /// remote endpoint's clock. That clock may not be synchronized with the
    /// local clock.
    ///
    /// [HIGRES-TIME]: https://www.w3.org/TR/webrtc-stats/#bib-highres-time
    pub remote_timestamp: Option<HighResTimeStamp>,

    /// Represents the total number of RTCP SR blocks sent for this SSRC.
    pub reports_sent: Option<u64>,
}

/// Statistics for the remote endpoint's inbound RTP stream corresponding
/// to an outbound stream that is currently sent with this
/// `RTCPeerConnection` object. It is measured at the remote endpoint and
/// reported in an RTCP Receiver Report (RR) or RTCP Extended Report (XR).
///
/// [`RtcStatsType::RemoteInboundRtp`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInboundRtpStreamStat {
    /// The `localId` is used for looking up the local
    /// [`RTCOutboundRtpStreamStats`] object for the same SSRC.
    ///
    /// [`RTCOutBoundRtpStreamStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#dom-rtcoutboundrtpstreamstats
    pub local_id: Option<String>,

    /// Estimated round trip time for this SSRC based on the RTCP timestamps in
    /// the RTCP Receiver Report (RR) and measured in seconds. Calculated as
    /// defined in section 6.4.1. of [RFC3550]. If no RTCP Receiver Report is
    /// received with a DLSR value other than 0, the round trip time is left
    /// undefined.
    ///
    /// [RFC3550]: https://www.w3.org/TR/webrtc-stats/#bib-rfc3550
    pub round_trip_time: Option<HighResTimeStamp>,

    /// The fraction packet loss reported for this SSRC. Calculated as defined
    /// in [RFC3550] section 6.4.1 and Appendix A.3.
    ///
    /// [RFC3550]: https://www.w3.org/TR/webrtc-stats/#bib-rfc3550
    pub fraction_lost: Option<Float>,

    /// Represents the total number of RTCP RR blocks received for this SSRC.
    pub reports_received: Option<u64>,

    /// Represents the total number of RTCP RR blocks received for this SSRC
    /// that contain a valid round trip time. This counter will increment if
    /// the roundTripTime is undefined.
    pub round_trip_time_measurements: Option<Float>,
}

/// An RTCRtpTransceiverStats stats object represents an RTCRtpTransceiver of
/// an RTCPeerConnection.
///
/// It appears as soon as the monitored RTCRtpTransceiver object is created,
/// such as by invoking addTransceiver, addTrack or setRemoteDescription.
/// RTCRtpTransceiverStats objects can only be deleted if the corresponding
/// RTCRtpTransceiver is removed - this can only happen if a remote description
/// is rolled back.
///
/// [`RtcStatsType::Transceiver`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#transceiver-dict*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub struct RtcRtpTransceiverStats {
    /// The identifier of the stats object representing the RTCRtpSender
    /// [associated with the `RTCRtpTransceiver`][1] represented by this stats
    /// object.
    ///
    /// [1]: https://w3c.github.io/webrtc-pc/#dom-rtcrtptransceiver-sender
    pub sender_id: Option<String>,

    /// The identifier of the stats object representing the RTCRtpReceiver
    /// [associated with the `RTCRtpTransceiver`][1] represented by this stats
    /// object.
    ///
    /// [1]: https://w3c.github.io/webrtc-pc/#dom-rtcrtptransceiver-receiver
    pub receiver_id: Option<String>,

    /// If the RTCRtpTransceiver that this stats object represents has a `mid`
    /// value that is not null, this is that value, otherwise this value is
    /// undefined.
    pub mid: Option<String>,
}

/// An [`RtcSctpTransportStats`] object represents the stats corresponding to an
/// `RTCSctpTransport`.
///
/// [`RtcStatsType::SctpTransport`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#sctptransportstats-dict*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct RtcSctpTransportStats {
    /// The latest smoothed round-trip time value, corresponding to
    /// `spinfo_srtt` defined in [RFC6458] but converted to seconds. If
    /// there has been no round-trip time measurements yet, this value is
    /// undefined.
    ///
    /// [RFC6458]: https://www.w3.org/TR/webrtc-stats/#bib-rfc6458
    pub smoothed_round_trip_time: Option<HighResTimeStamp>,
}

/// An [`RtcTransportStats`] object represents the stats corresponding to an
/// [`RTCDtlsTransport`] and its underlying [`RTCIceTransport`]. When RTCP
/// multiplexing is used, one transport is used for both RTP and RTCP.
/// Otherwise, RTP and RTCP will be sent on separate transports, and
/// `rtcpTransportStatsId` can be used to pair the resulting
/// [`RtcTransportStats`] objects. Additionally, when bundling is used, a single
/// transport will be used for all [`MediaStreamTrack`]s in the bundle group. If
/// bundling is not used, different [`MediaStreamTrack`] will use different
/// transports. RTCP multiplexing and bundling are described in [WEBRTC].
///
/// [`RtcStatsType::Transport`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#transportstats-dict*
/// [`RTCDtlsTransport`]:
/// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdtlstransport
/// [`RTCIceTransport`]: https://www.w3.org/TR/webrtc-stats/#dfn-rtcicetransport
/// [`MediaStreamTrack`]:
/// https://www.w3.org/TR/webrtc-stats/#dfn-mediastreamtrack
/// [WEBRTC]: https://www.w3.org/TR/webrtc-stats/#bib-webrtc
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct RtcTransportStats {
    /// Represents the total number of packets sent over this transport.
    pub packets_sent: Option<u64>,

    /// Represents the total number of packets received on this transport.
    pub packets_received: Option<u64>,

    /// Represents the total number of payload bytes sent on this
    /// `PeerConnection`, i.e., not including headers or padding.
    pub bytes_sent: Option<u64>,

    /// Represents the total number of bytes received on this PeerConnection,
    /// i.e., not including headers or padding.
    pub bytes_received: Option<u64>,

    /// Set to the current value of the "role" attribute of the underlying
    /// RTCDtlsTransport's "transport".
    pub ice_role: Option<IceRole>,
}

/// Variants of ICE roles.
///
/// More info in the [RFC5245].
///
/// [RFC5245]: https://tools.ietf.org/html/rfc5245
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub enum IceRole {
    /// An agent whose role as defined by [ICE], Section 3, has not yet been
    /// determined.
    ///
    /// [ICE]: https://www.w3.org/TR/webrtc/#bib-ice
    Unknown,

    /// A controlling agent as defined by [ICE], Section 3.
    ///
    /// [ICE]: https://www.w3.org/TR/webrtc/#bib-ice
    Controlling,

    /// A controlled agent as defined by [ICE], Section 3.
    ///
    /// [ICE]: https://www.w3.org/TR/webrtc/#bib-ice
    Controlled,
}

/// Statistics related to a specific `RTCRtpSender` and the corresponding
/// media-level metrics.
///
/// [`RtcStatsType::Sender`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#dom-rtcstatstype-sender
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(tag = "kind")]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub enum SenderStatsKind {
    /// Represents [`RTCAudioSenderStats`] object.
    ///
    /// [`RTCAudioSenderStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#obsolete-rtcaudiosenderstats-members
    Audio { media_source_id: Option<String> },

    /// Represents [`RTCVideoSenderStats`] object.
    ///
    /// [`RTCVideoSenderStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#obsolete-rtcvideosenderstats-members
    Video { media_source_id: Option<String> },
}

/// Statistics related to a specific receiver and the corresponding
/// media-level metrics.
///
/// [`RtcStatsType::Receiver`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#dom-rtcstatstype-receiver
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(tag = "kind")]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub enum ReceiverStatsKind {
    /// Represents [`RTCAudioReceiverStats`] object.
    ///
    /// [`RTCAudioReceiverStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#dom-rtcaudioreceiverstats
    Audio {},

    /// Represents [`RTCVideoReceiverStats`] object.
    ///
    /// [`RTCVideoReceiverStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#rvststats-dict*
    Video {},
}

/// ICE candidate pair statistics related to the `RTCIceTransport` objects.
///
/// A candidate pair that is not the current pair for a transport is
/// [deleted] when the `RTCIceTransport` does an ICE restart, at the time
/// the state changes to "new". The candidate pair that is the current
/// pair for a transport is deleted after an ICE restart when the
/// `RTCIceTransport` switches to using a candidate pair generated from
/// the new candidates; this time doesn't correspond to any other
/// externally observable event.
///
/// [`RtcStatsType::CandidatePair`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#candidatepair-dict*
/// [deleted]: https://www.w3.org/TR/webrtc-stats/#dfn-deleted
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct RtcIceCandidatePairStats {
    /// Represents the state of the checklist for the local and remote
    /// candidates in a pair.
    pub state: IceCandidatePairState,

    /// Related to updating the nominated flag described in Section 7.1.3.2.4
    /// of [RFC5245].
    ///
    /// [RFC5245]: https://www.w3.org/TR/webrtc-stats/#bib-rfc5245
    pub nominated: bool,

    /// Represents the total number of payload bytes sent on this candidate
    /// pair, i.e., not including headers or padding.
    pub bytes_sent: u64,

    /// Represents the total number of payload bytes received on this candidate
    /// pair, i.e., not including headers or padding.
    pub bytes_received: u64,

    /// Represents the sum of all round trip time measurements in seconds since
    /// the beginning of the session, based on STUN connectivity check
    /// [STUN-PATH-CHAR] responses (responsesReceived), including those that
    /// reply to requests that are sent in order to verify consent [RFC7675].
    /// The average round trip time can be computed from `totalRoundTripTime`
    /// by dividing it by `responsesReceived`.
    ///
    /// [STUN-PATH-CHAR]: https://www.w3.org/TR/webrtc-stats/#bib-stun-path-char
    /// [RFC7675]: https://www.w3.org/TR/webrtc-stats/#bib-rfc7675
    pub total_round_trip_time: Option<HighResTimeStamp>,

    /// Represents the latest round trip time measured in seconds, computed
    /// from both STUN connectivity checks [STUN-PATH-CHAR], including those
    /// that are sent for consent verification [RFC7675].
    ///
    /// [STUN-PATH-CHAR]: https://www.w3.org/TR/webrtc-stats/#bib-stun-path-char
    /// [RFC7675]: https://www.w3.org/TR/webrtc-stats/#bib-rfc7675
    pub current_round_trip_time: Option<HighResTimeStamp>,

    /// It is calculated by the underlying congestion control by combining the
    /// available bitrate for all the outgoing RTP streams using this candidate
    /// pair. The bitrate measurement does not count the size of the IP or
    /// other transport layers like TCP or UDP. It is similar to the TIAS
    /// defined in [RFC3890], i.e., it is measured in bits per second and the
    /// bitrate is calculated over a 1 second window.
    ///
    /// Implementations that do not calculate a sender-side estimate MUST leave
    /// this undefined. Additionally, the value MUST be undefined for candidate
    /// pairs that were never used. For pairs in use, the estimate is normally
    /// no lower than the bitrate for the packets sent at
    /// `lastPacketSentTimestamp`, but might be higher. For candidate pairs
    /// that are not currently in use but were used before, implementations
    /// MUST return undefined.
    ///
    /// [RFC3890]: https://www.w3.org/TR/webrtc-stats/#bib-rfc3890
    pub available_outgoing_bitrate: Option<u64>,
}

/// Each candidate pair in the check list has a foundation and a state.
/// The foundation is the combination of the foundations of the local and
/// remote candidates in the pair.  The state is assigned once the check
/// list for each media stream has been computed.  There are five
/// potential values that the state can have.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum KnownIceCandidatePairState {
    /// A check has not been performed for this pair, and can be
    /// performed as soon as it is the highest-priority Waiting pair on
    /// the check list.
    Waiting,

    /// A check has been sent for this pair, but the transaction
    /// is in progress.
    InProgress,

    /// A check for this pair was already done and produced a
    /// successful result.
    Succeeded,

    /// A check for this pair was already done and failed, either never
    /// producing any response or producing an unrecoverable failure
    /// response.
    Failed,

    /// A check for this pair hasn't been performed, and it can't
    /// yet be performed until some other check succeeds, allowing this
    /// pair to unfreeze and move into the Waiting state.
    Frozen,
}

/// Stores known [`KnownIceCandidatePairState`] (as [`NonExhaustive::Known`]
/// enum variant) or unknown state (as [`NonExhaustive::Unknown`] enum variant).
pub type IceCandidatePairState = NonExhaustive<KnownIceCandidatePairState>;

/// Known protocols used in the WebRTC.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "lowercase")]
pub enum KnownProtocol {
    /// User Datagram Protocol.
    ///
    /// [Wiki][1]
    ///
    /// [1]: https://en.wikipedia.org/wiki/User_Datagram_Protocol
    Udp,

    /// Transmission Control Protocol
    ///
    /// [Wiki][1]
    ///
    /// [1]: https://en.wikipedia.org/wiki/Transmission_Control_Protocol
    Tcp,
}

/// Stores known [`KnownProtocol`] (as [`NonExhaustive::Known`] enum
/// variant) or unknown state (as [`NonExhaustive::Unknown`] enum variant).
pub type Protocol = NonExhaustive<KnownProtocol>;

/// The `RTCIceCandidateType` represents the type of the ICE candidate, as
/// defined in [ICE] section 15.1.
///
/// [ICE]: https://www.w3.org/TR/webrtc/#bib-ice
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "lowercase")]
pub enum KnownCandidateType {
    /// A host candidate, as defined in Section 4.1.1.1 of [ICE].
    ///
    /// [ICE]: https://www.w3.org/TR/webrtc/#bib-ice
    Host,

    /// A server reflexive candidate, as defined in Section 4.1.1.2 of [ICE].
    ///
    /// [ICE]: https://www.w3.org/TR/webrtc/#bib-ice
    Srlfx,

    /// A peer reflexive candidate, as defined in Section 4.1.1.2 of [ICE].
    ///
    /// [ICE]: https://www.w3.org/TR/webrtc/#bib-ice
    Prflx,

    /// A relay candidate, as defined in Section 7.1.3.2.1 of [ICE].
    ///
    /// [ICE]: https://www.w3.org/TR/webrtc/#bib-ice
    Relay,
}

/// Stores known [`KnownCandidateType`] (as [`NonExhaustive::Known`] enum
/// variant) or unknown state (as [`NonExhaustive::Unknown`] enum variant).
pub type CandidateType = NonExhaustive<KnownCandidateType>;

/// Fields which should be in the [`RtcStat`] of
/// [`RtcStatsType::InboundRtp`] type based on `mediaType`.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "mediaType")]
pub enum RtcInboundRtpStreamMediaType {
    /// Will be deserialized when `mediaType` is `audio`.
    Audio {
        /// Whether the last RTP packet whose frame was delivered to the
        /// `RTCRtpReceiver`'s `MediaStreamTrack` for playout contained voice
        /// activity or not based on the presence of the V bit in the extension
        /// header, as defined in [RFC6464].
        ///
        /// [RFC6464]: https://www.w3.org/TR/webrtc-stats/#bib-rfc6464
        voice_activity_flag: Option<bool>,

        /// The total number of samples that have been received on this RTP
        /// stream. This includes `concealedSamples`.
        total_samples_received: Option<u64>,

        /// The total number of samples that are concealed samples. A concealed
        /// sample is a sample that was replaced with synthesized samples
        /// generated locally before being played out. Examples of samples that
        /// have to be concealed are samples from lost packets (reported in
        /// `packetsLost`) or samples from packets that arrive too late to be
        /// played out (reported in `packetsDiscarded`).
        concealed_samples: Option<u64>,

        /// The total number of concealed samples inserted that are "silent".
        /// Playing out silent samples results in silence or comfort noise.
        /// This is a subset of `concealedSamples`.
        silent_concealed_samples: Option<u64>,

        /// Represents the audio level of the receiving track.
        audio_level: Option<Float>,

        /// Represents the audio energy of the receiving track.
        total_audio_energy: Option<Float>,

        /// Represents the audio duration of the receiving track. For audio
        /// durations of tracks attached locally, see RTCAudioSourceStats
        /// instead.
        total_samples_duration: Option<HighResTimeStamp>,
    },

    /// Will be deserialized when `mediaType` is `video`.
    Video {
        /// It represents the total number of frames correctly decoded for this
        /// RTP stream, i.e., frames that would be displayed if no frames are
        /// dropped.
        frames_decoded: Option<u64>,

        /// It represents the total number of key frames, such as key frames in
        /// VP8 [RFC6386] or IDR-frames in H.264 [RFC6184], successfully
        /// decoded for this RTP media stream. This is a subset of
        /// framesDecoded. `framesDecoded - keyFramesDecoded` gives you the
        /// number of delta frames decoded.
        ///
        /// [RFC6385]: https://www.w3.org/TR/webrtc-stats/#bib-rfc6386
        /// [RFC6184]: https://www.w3.org/TR/webrtc-stats/#bib-rfc6184
        key_frames_decoded: Option<u64>,

        /// Represents the width of the last decoded frame. Before the first
        /// frame is decoded this attribute is missing.
        frame_width: Option<u64>,

        /// Represents the height of the last decoded frame. Before the first
        /// frame is decoded this attribute is missing.
        frame_height: Option<u64>,

        /// Sum of the interframe delays in seconds between consecutively
        /// decoded frames, recorded just after a frame has been decoded.
        total_inter_frame_delay: Option<Float>,

        /// The number of decoded frames in the last second.
        #[serde(rename = "framesPerSecond")]
        fps: Option<u64>,

        /// Represents the bit depth per pixel of the last decoded frame.
        /// Typical values are 24, 30, or 36 bits. Before the first frame is
        /// decoded this attribute is missing.
        frame_bit_depth: Option<u64>,

        /// Count the total number of Full Intra Request (FIR) packets sent by
        /// this receiver.
        fir_count: Option<u64>,

        /// Count the total number of Picture Loss Indication (PLI) packets
        /// sent by this receiver.
        pli_count: Option<u64>,

        /// Count the total number of Slice Loss Indication (SLI) packets sent
        /// by this receiver.
        sli_count: Option<u64>,

        /// The number of concealment events. This counter increases every
        /// time a concealed sample is synthesized after a non-concealed
        /// sample. That is, multiple consecutive concealed samples will
        /// increase the `concealedSamples` count multiple times but is a
        /// single concealment event.
        concealment_events: Option<u64>,

        /// Represents the total number of complete frames received on this RTP
        /// stream. This metric is incremented when the complete frame is
        /// received. Represents the total number of complete frames received
        /// on this RTP stream. This metric is incremented when the complete
        /// frame is received.
        frames_received: Option<u64>,
    },
}

/// The [`RtcInboundRtpStreamStats`] dictionary represents the measurement
/// metrics for the incoming RTP media stream. The timestamp reported in the
/// statistics object is the time at which the data was sampled.
///
/// [`RtcStatsType::InboundRtp`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct RtcInboundRtpStreamStats {
    /// The identifier of the stats object representing the receiving track.
    pub track_id: Option<String>,

    /// Fields which should be in the [`RtcStat`] based on `mediaType`.
    #[serde(flatten)]
    pub media_specific_stats: RtcInboundRtpStreamMediaType,

    /// Total number of bytes received for this SSRC.
    pub bytes_received: u64,

    /// Total number of RTP packets received for this SSRC.
    pub packets_received: u64,

    /// Total number of RTP packets lost for this SSRC.
    pub packets_lost: Option<u64>,

    /// Packet Jitter measured in seconds for this SSRC.
    pub jitter: Option<Float>,

    /// Total number of seconds that have been spent decoding the
    /// `framesDecoded` frames of this stream. The average decode time can
    /// be calculated by dividing this value with `framesDecoded`. The time
    /// it takes to decode one frame is the time passed between feeding the
    /// decoder a frame and the decoder returning decoded data for that
    /// frame.
    pub total_decode_time: Option<HighResTimeStamp>,

    /// The total number of audio samples or video frames that have come out of
    /// the jitter buffer (increasing `jitterBufferDelay`).
    pub jitter_buffer_emitted_count: Option<u64>,
}

/// Statistics related to a specific `MediaStreamTrack`'s attachment to an
/// `RTCRtpSender` and the corresponding media-level metrics.
///
/// [`RtcStatsType::Track`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#dom-rtcstatstype-track
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TrackStat {
    /// Represents the `id` property of the track.
    #[serde(rename = "trackIdentifier")]
    pub track_id: String,

    /// True if the source is remote, for instance if it is sourced from
    /// another host via an RTCPeerConnection. False otherwise.
    pub remote_source: Option<bool>,

    /// Reflects the "ended" state of the track.
    pub ended: Option<bool>,

    /// Either `audio` or `video`.
    ///
    /// This reflects the `kind` attribute of the `MediaStreamTrack`.
    pub kind: Option<String>,
}

/// Reflects the `kind` attribute of the `MediaStreamTrack`.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub enum TrackStatKind {
    /// `MediaStreamTrack` is used for the audio content.
    Audio,

    /// `MediaStreamTrack` is used for the video content.
    Video,
}

/// Fields which should be in the [`RtcStat`] of
/// [`RtcStatsType::OutboundRtp`] type based on `mediaType`.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "mediaType")]
pub enum RtcOutboundRtpStreamMediaType {
    /// Will be deserialized when `mediaType` is `audio`.
    Audio {
        /// The total number of samples that have been sent over this RTP
        /// stream.
        total_samples_sent: Option<u64>,

        /// Whether the last RTP packet sent contained voice activity or not
        /// based on the presence of the V bit in the extension header
        voice_activity_flag: Option<bool>,
    },

    /// Will be deserialized when `mediaType` is `audio`.
    Video {
        /// Represents the width of the last encoded frame. The resolution of
        /// the encoded frame may be lower than the media source (see
        /// `RTCVideoSourceStats.width`). Before the first frame is encoded
        /// this attribute is missing.
        frame_width: Option<u64>,

        /// Represents the height of the last encoded frame. The resolution of
        /// the encoded frame may be lower than the media source (see
        /// `RTCVideoSourceStats.height`). Before the first frame is encoded
        /// this attribute is missing.
        frame_height: Option<u64>,

        /// The number of encoded frames during the last second. This may be
        /// lower than the media source frame rate (see
        /// `RTCVideoSourceStats.framesPerSecond`).
        frames_per_second: Option<u64>,
    },
}

/// Statistics for an outbound RTP stream that is currently sent with this
/// `RTCPeerConnection` object. It is accessed by the
/// [`RtcOutboundRtpStreamStats`].
///
/// When there are multiple RTP streams connected to the same sender, such
/// as when using simulcast or RTX, there will be one
/// [`RtcOutboundRtpStreamStats`] per RTP stream, with distinct values of
/// the "ssrc" attribute, and all these senders will have a reference to
/// the same "sender" object (of type `RTCAudioSenderStats` or
/// `RTCVideoSenderStats`) and "track" object (of type
/// `RTCSenderAudioTrackAttachmentStats` or
/// `RTCSenderVideoTrackAttachmentStats`).
///
/// [`RtcStatsType::OutboundRtp`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#outboundrtpstats-dict*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct RtcOutboundRtpStreamStats {
    /// The identifier of the stats object representing the current track
    /// attachment to the sender of this stream.
    pub track_id: Option<String>,

    /// Fields which should be in the [`RtcStat`] based on `mediaType`.
    #[serde(flatten)]
    pub media_type: RtcOutboundRtpStreamMediaType,

    /// Total number of bytes sent for this SSRC.
    pub bytes_sent: u64,

    /// Total number of RTP packets sent for this SSRC.
    pub packets_sent: u64,

    /// The identifier of the stats object representing the track currently
    /// attached to the sender of this stream.
    pub media_source_id: Option<String>,
}

/// `RTCIceCandidateStats` reflects the properties of a `candidate` in Section
/// 15.1 of [RFC5245]. It corresponds to a `RTCIceCandidate` object.
///
/// [`RtcStatsType::LocalCandidate`] or
/// [`RtcStatsType::RemoteCandidate`] variants.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#icecandidate-dict*
/// [RFC5245]: https://www.w3.org/TR/webrtc-stats/#bib-rfc5245
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct RtcIceCandidateStats {
    /// It is a unique identifier that is associated to the object that was
    /// inspected to produce the `RTCTransportStats` associated with this
    /// candidate.
    pub transport_id: Option<String>,

    /// It is the address of the candidate, allowing for IPv4 addresses, IPv6
    /// addresses, and fully qualified domain names (FQDNs).
    pub address: Option<String>,

    /// It is the port number of the candidate.
    pub port: u16,

    /// Valid values for transport is one of `udp` and `tcp`.
    pub protocol: Protocol,

    /// The `RTCIceCandidateType` represents the type of the ICE candidate.
    pub candidate_type: CandidateType,

    /// Calculated as defined in [RFC5245] section 15.1.
    ///
    /// [RFC5245]: https://www.w3.org/TR/webrtc-stats/#bib-rfc5245
    pub priority: u32,

    /// For local candidates this is the URL of the ICE server from which the
    /// candidate was obtained. It is the same as the [url surfaced in the
    /// `RTCPeerConnectionIceEvent`][1].
    ///
    /// `None` for remote candidates.
    ///
    /// [1]: https://w3c.github.io/webrtc-pc/#rtcpeerconnectioniceevent
    pub url: Option<String>,

    /// It is the protocol used by the endpoint to communicate with the TURN
    /// server. This is only present for local candidates.
    pub relay_protocol: Option<Protocol>,
}

/// Fields which should be in the [`RtcStat`] of
/// [`RtcStatsType::MediaSource`] type based on `kind`.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "kind")]
pub enum MediaSourceKind {
    /// Will be deserialized when `kind` is `video`.
    Video {
        /// The width, in pixels, of the last frame originating from this
        /// source. Before a frame has been produced this attribute is missing.
        width: Option<u32>,

        /// The height, in pixels, of the last frame originating from this
        /// source. Before a frame has been produced this attribute is missing.
        height: Option<u32>,

        /// The number of frames originating from this source, measured during
        /// the last second. For the first second of this object's lifetime
        /// this attribute is missing.
        #[serde(rename = "framesPerSecond")]
        fps: Option<u32>,
    },

    /// Will be deserialized when `kind` is `audio`.
    Audio {
        /// Represents the audio level of the media source.
        audio_level: Option<Float>,

        /// Represents the audio energy of the media source.
        total_audio_energy: Option<Float>,

        /// Represents the audio duration of the media source.
        total_samples_duration: Option<Float>,
    },
}

/// Statistics for the media produced by a MediaStreamTrack that is currently
/// attached to an `RTCRtpSender`.
///
/// [`RtcStatsType::MediaSource`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#dom-rtcstatstype-media-source
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
pub struct MediaSourceStat {
    /// The value of the `MediaStreamTrack`'s ID attribute.
    #[serde(rename = "trackIdentifier")]
    pub track_id: Option<String>,

    /// Fields which should be in the [`RtcStat`] based on `kind`.
    #[serde(flatten)]
    pub kind: MediaSourceKind,
}

/// Statistics for a codec that is currently being used by RTP streams
/// being sent or received by this `RTCPeerConnection` object. It is
/// accessed by the [`RtcCodecStats`].
///
/// [`RtcStatsType::Codec`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#dom-rtccodecstats
#[cfg(feature = "unused")]
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub struct RtcCodecStats {
    /// Payload type as used in RTP encoding or decoding.
    pub payload_type: u32,

    /// The codec MIME media type/subtype. e.g., video/vp8 or equivalent.
    pub mime_type: String,

    /// Represents the media sampling rate.
    pub clock_rate: u32,
}

/// Information about a certificate used by an `RTCIceTransport`.
///
/// [`RtcStatsType::Certificate`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#certificatestats-dict*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub struct RtcCertificateStats {
    /// The fingerprint of the certificate. Only use the fingerprint value as
    /// defined in Section 5 of [RFC4572].
    ///
    /// [RFC4572]: https://www.w3.org/TR/webrtc-stats/#bib-rfc4572
    pub fingerprint: String,

    /// The hash function used to compute the certificate fingerprint. For
    /// instance, "sha-256".
    pub fingerprint_algorithm: String,

    /// The DER-encoded base-64 representation of the certificate.
    pub base64_certificate: String,
}

/// Represents [`DOMHighResTimeStamp`].
///
/// Can be converted to the [`SystemTime`] with accurate to the milliseconds.
///
/// The [`HighResTimeStamp`] type is a [`f32`] and is used to store a time value
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
/// [`DOMHighResTimeStamp`]:
/// https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HighResTimeStamp(pub f64);

impl From<HighResTimeStamp> for SystemTime {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn from(timestamp: HighResTimeStamp) -> Self {
        SystemTime::UNIX_EPOCH + Duration::from_millis(timestamp.0 as u64)
    }
}

impl Hash for HighResTimeStamp {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_string().hash(state);
    }
}

impl PartialEq for HighResTimeStamp {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string().eq(&other.0.to_string())
    }
}

/// Wrapped around [`f64`] for which implemented [`Hash`].
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Float(pub f64);

impl Hash for Float {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // TODO: add some docs
        self.0.to_string().hash(state);
    }
}

impl PartialEq for Float {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string().eq(&other.0.to_string())
    }
}

/// Information about the connection to an ICE server (e.g. STUN or TURN).
///
/// [`RtcStatsType::IceServer`] variant.
///
/// [Full doc on W3C][1]
///
/// [1]: https://www.w3.org/TR/webrtc-stats/#ice-server-dict*
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Hash)]
#[serde(rename_all = "camelCase")]
#[cfg(feature = "unused")]
pub struct RtcIceServerStats {
    /// The URL of the ICE server (e.g. TURN or STUN server).
    pub url: String,

    /// It is the port number used by the client.
    pub port: u16,

    /// Valid values for transport is one of udp and tcp.
    pub protocol: Protocol,

    /// The total amount of requests that have been sent to this server.
    pub total_requests_sent: Option<u64>,

    /// The total amount of responses received from this server.
    pub total_responses_received: Option<u64>,

    /// The sum of RTTs for all requests that have been sent where a response
    /// has been received.
    pub total_round_trip_time: Option<HighResTimeStamp>,
}
