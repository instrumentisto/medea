//! [Spec][] is quite new atm, and is poorly adopted by UA's.
//!
//! [RTCStatsReport][2] allows [maplike][3] operations. [entries()][4] operation
//! returns array of arrays, where first value is [RTCStats.id][5] and second is
//! actual [RTCStats][6].
//!
//! [1]: https://www.w3.org/TR/webrtc-stats/
//! [2]: https://www.w3.org/TR/webrtc/#rtcstatsreport-object
//! [3]: https://heycam.github.io/webidl/#idl-maplike
//! [4]: https://heycam.github.io/webidl/#es-map-entries
//! [5]: https://www.w3.org/TR/webrtc/#dom-rtcstats-id
//! [6]: https://www.w3.org/TR/webrtc/#dom-rtcstats
//!

use std::convert::TryFrom;

use serde::Deserialize;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::RtcStats as SysRtcStats;

use crate::utils::{console_error, get_property_by_name};
use futures::future::Remote;
use std::{collections::HashMap, time::Duration};

struct RtcStatsReportEntry(js_sys::JsString, SysRtcStats);

impl TryFrom<js_sys::Array> for RtcStatsReportEntry {
    type Error = ();

    fn try_from(value: js_sys::Array) -> Result<Self, Self::Error> {
        let id = value.get(0);
        let stats = value.get(1);

        if id.is_undefined() {
            panic!("asdasd");
        }

        if stats.is_undefined() {
            panic!("asdasd2222");
        }

        let id = id.dyn_into::<js_sys::JsString>().unwrap();
        let stats = stats.dyn_into::<SysRtcStats>().unwrap();

        Ok(RtcStatsReportEntry(id, stats))
    }
}

#[derive(Debug, Deserialize)]
pub struct RtcStat<T> {
    id: String,
    timestamp: f32,
    #[serde(flatten)]
    kind: T,
}

#[derive(Debug)]
pub struct RtcStats(Vec<RtcStatsType>);

impl From<&JsValue> for RtcStats {
    fn from(stats: &JsValue) -> Self {
        let entries_fn =
            get_property_by_name(&stats, "entries", |func: JsValue| {
                Some(func.unchecked_into::<js_sys::Function>())
            })
            .unwrap();

        let iterator = entries_fn
            .call0(stats.as_ref())
            .unwrap()
            .unchecked_into::<js_sys::Iterator>();

        let mut stats = Vec::new();

        let mut next = iterator.next().unwrap();
        while !next.done() {
            let stat = next.value();
            let stat = stat.unchecked_into::<js_sys::Array>();
            let stat = RtcStatsReportEntry::try_from(stat).unwrap();
            let stat = RtcStatsType::try_from(&stat.1).unwrap();

            stats.push(stat);

            next = iterator.next().unwrap();
        }

        RtcStats(stats)
    }
}

/// https://www.w3.org/TR/webrtc-stats/#rtctatstype-*
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "kebab-case")]
enum KnownRtcStatsType {
    Codec(RtcStat<CodecStat>),
    InboundRtp(RtcStat<RtcInboundRtpStreamStats>),
    OutboundRtp(RtcStat<OutboundRtpStats>),
    RemoteInboundRtp(RtcStat<RemoteInboundRtpStreamStat>),
    RemoteOutboundRtp(RtcStat<RemoteOutboundRtpStreamStat>),
    Csrc(RtcStat<RtpContributingSourceStat>),
    PeerConnection(RtcStat<RtcPeerConnectionStat>),
    DataChannel(RtcStat<DataChannelStat>),
    Stream(RtcStat<MediaStreamStat>),
    Track(RtcStat<TrackStat>),
    Transceiver(RtcStat<RtpTransceiverStat>),
    Sender(RtcStat<SenderStatsKind>),
    Receiver(RtcStat<ReceiverStatsKind>),
    Transport(RtcStat<TransportStat>),
    SctpTransport(RtcStat<SctpTransport>),
    CandidatePair(RtcStat<RtcIceCandidatePairStats>),
    LocalCandidate(RtcStat<LocalCandidateStat>),
    RemoteCandidate(RtcStat<RemoteCandidateStat>),
    Certificate(RtcStat<CertificateStat>),
    IceServer(RtcStat<IceServerStat>),
    MediaSource(RtcStat<MediaSourceStat>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaStreamStat {
    /// `stream.id` property.
    #[serde(rename = "streamIdentifier")]
    stream_id: String,

    /// This is the id of the stats object, not the `track.id`.
    track_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataChannelStat {
    /// The "label" value of the [`RTCDataChannel`] object.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    label: Option<String>,

    /// The "protocol" value of the [`RTCDataChannel`] object.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    protocol: Option<Protocol>,

    /// The "id" attribute of the [`RTCDataChannel`] object.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    #[serde(rename = "dataChannelIdentifier")]
    data_channel_id: Option<f32>,

    /// A [stats object reference] for the transport used to carry this
    /// datachannel.
    ///
    /// [stats object reference]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-stats-object-reference
    transport_id: Option<String>,

    /// The "readyState" value of the [`RTCDataChannel`] object.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    state: Option<DataChannelState>,

    /// Represents the total number of API "message" events sent.
    messages_sent: Option<f32>,

    /// Represents the total number of payload bytes sent on this
    /// [`RTCDataChannel`], i.e., not including headers or padding.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    bytes_sent: Option<f64>,

    /// Represents the total number of API "message" events received.
    messages_received: Option<f32>,

    /// Represents the total number of bytes received on this
    /// [`RTCDataChannel`], i.e., not including headers or padding.
    ///
    /// [`RTCDataChannel`]:
    /// https://www.w3.org/TR/webrtc-stats/#dfn-rtcdatachannel
    bytes_received: Option<f64>,
}

pub type DataChannelState = NonExhaustive<KnownDataChannelState>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownDataChannelState {
    Connecting,
    Open,
    Closing,
    Closed,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtcPeerConnectionStat {
    /// Represents the number of unique `DataChannel`s that have entered the
    /// "open" state during their lifetime.
    data_channels_opened: Option<f32>,

    /// Represents the number of unique `DataChannel`s that have left the
    /// "open" state during their lifetime (due to being closed by either
    /// end or the underlying transport being closed). `DataChannel`s that
    /// transition from "connecting" to "closing" or "closed" without ever
    /// being "open" are not counted in this number.
    data_channels_closed: Option<f32>,

    /// Represents the number of unique `DataChannel`s returned from a
    /// successful `createDataChannel()` call on the `RTCPeerConnection`.
    /// If the underlying data transport is not established, these may be
    /// in the "connecting" state.
    data_channels_requested: Option<f32>,

    /// Represents the number of unique `DataChannel`s signaled in a
    /// "datachannel" event on the `RTCPeerConnection`.
    data_channels_accepted: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtpContributingSourceStat {
    /// The SSRC identifier of the contributing source represented by this
    /// stats object, as defined by [RFC3550]. It is a 32-bit unsigned integer
    /// that appears in the CSRC list of any packets the relevant source
    /// contributed to.
    ///
    /// [RFC3550]: https://www.w3.org/TR/webrtc-stats/#bib-rfc3550
    contributor_ssrc: Option<f32>,

    /// The ID of the [`RTCInboundRtpStreamStats`] object representing the
    /// inbound RTP stream that this contributing source is contributing to.
    ///
    /// [`RTCInboundRtpStreamStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
    inbound_rtp_stream_id: Option<String>,

    /// The total number of RTP packets that this contributing source
    /// contributed to. This value is incremented each time a packet is counted
    /// by [`RTCInboundRtpStreamStats.packetsReceived`], and the packet's CSRC
    /// list (as defined by [RFC3550] section 5.1) contains the SSRC identifier
    /// of this contributing source, [`contributorSsrc`].
    ///
    /// [`RTCInboundRtpStreamStats.packetsReceived`]:
    /// https://tinyurl.com/rreuf49
    /// [`contributorSsrc`]: https://tinyurl.com/tf8c7j4
    packets_contributed_to: Option<f32>,

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
    audio_level: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteOutboundRtpStreamStat {
    /// The `localId` is used for looking up the local
    /// [`RTCInboundRtpStreamStats`] object for the same SSRC.
    ///
    /// [`RTCInboundRtpStreamStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#dom-rtcinboundrtpstreamstats
    local_id: Option<String>,

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
    // TODO: test that this is correct.
    remote_timestamp: Option<Duration>,

    /// Represents the total number of RTCP SR blocks sent for this SSRC.
    reports_sent: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInboundRtpStreamStat {
    /// The `localId` is used for looking up the local
    /// [`RTCOutboundRtpStreamStats`] object for the same SSRC.
    ///
    /// [`RTCOutBoundRtpStreamStats`]:
    /// https://www.w3.org/TR/webrtc-stats/#dom-rtcoutboundrtpstreamstats
    local_id: Option<String>,

    /// Estimated round trip time for this SSRC based on the RTCP timestamps in
    /// the RTCP Receiver Report (RR) and measured in seconds. Calculated as
    /// defined in section 6.4.1. of [RFC3550]. If no RTCP Receiver Report is
    /// received with a DLSR value other than 0, the round trip time is left
    /// undefined.
    ///
    /// [RFC3550]: https://www.w3.org/TR/webrtc-stats/#bib-rfc3550
    round_trip_time: Option<f32>,

    /// The fraction packet loss reported for this SSRC. Calculated as defined
    /// in [RFC3550] section 6.4.1 and Appendix A.3.
    ///
    /// [RFC3550]: https://www.w3.org/TR/webrtc-stats/#bib-rfc3550
    fraction_lost: Option<f32>,

    /// Represents the total number of RTCP RR blocks received for this SSRC.
    reports_received: Option<f64>,

    /// Represents the total number of RTCP RR blocks received for this SSRC
    /// that contain a valid round trip time. This counter will increment if
    /// the roundTripTime is undefined.
    round_trip_time_measurements: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtpTransceiverStat {
    sender_id: Option<String>,
    receiver_id: Option<String>,
    mid: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SctpTransport {
    smoothed_round_trip_time: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportStat {
    /// Represents the total number of packets sent over this transport.
    packets_sent: Option<u64>,

    /// Represents the total number of packets received on this transport.
    packets_received: Option<u64>,

    /// Represents the total number of payload bytes sent on this
    /// `PeerConnection`, i.e., not including headers or padding.
    bytes_sent: Option<u64>,

    /// Represents the total number of bytes received on this PeerConnection,
    /// i.e., not including headers or padding.
    bytes_received: Option<u64>,

    /// Set to the current value of the "role" attribute of the underlying
    /// RTCDtlsTransport's "transport".
    ice_role: Option<IceRole>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IceRole {
    Unknown,
    Controlling,
    Controlled,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
#[serde(rename_all = "camelCase")]
pub enum SenderStatsKind {
    Audio { media_source_id: Option<String> },
    Video { media_source_id: Option<String> },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
#[serde(rename_all = "camelCase")]
pub enum ReceiverStatsKind {
    Audio {},
    Video {},
}

type RtcStatsType = NonExhaustive<KnownRtcStatsType>;

impl TryFrom<&SysRtcStats> for RtcStatsType {
    type Error = serde_json::Error;

    fn try_from(val: &SysRtcStats) -> Result<Self, Self::Error> {
        JsValue::from(val).into_serde()
    }
}

// https://www.w3.org/TR/webrtc-stats/#candidatepair-dict*
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RtcIceCandidatePairStats {
    state: IceCandidatePairState,
    nominated: bool,
    writable: bool,
    bytes_sent: u64,
    bytes_received: u64,
    total_round_trip_time: Option<f64>,
    current_round_trip_time: Option<f64>,
    available_outgoing_bitrate: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum NonExhaustive<T> {
    Known(T),
    Unknown(String),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum KnownIceCandidatePairState {
    Frozen,
    Waiting,
    InProgress,
    Failed,
    Succeeded,
}

type IceCandidatePairState = NonExhaustive<KnownIceCandidatePairState>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum KnownProtocol {
    Udp,
    Tcp,
}

type Protocol = NonExhaustive<KnownProtocol>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum KnownCandidateType {
    Prflx,
    Host,
}

type CandidateType = NonExhaustive<KnownCandidateType>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum KnownMediaType {
    Audio,
    Video,
}

type MediaType = NonExhaustive<KnownMediaType>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "mediaType")]
enum RtcInboundRtpStreamMediaType {
    Audio {
        voice_activity_flag: Option<bool>,
        total_samples_received: Option<u64>,
        concealed_samples: Option<u64>,
        silent_concealed_samples: Option<u64>,
        audio_level: Option<f32>,
        total_audio_energy: Option<f32>,
        total_samples_duration: Option<f32>,
    },
    Video {
        frames_decoded: Option<u64>,
        key_frames_decoded: Option<u64>,
        frame_width: Option<u64>,
        frame_height: Option<u64>,
        total_inter_frame_delay: Option<f32>,
        #[serde(rename = "framesPerSecond")]
        fps: Option<u64>,
        frame_bit_depth: Option<u64>,
        fir_count: Option<u64>,
        pli_count: Option<u64>,
        sli_count: Option<u64>,
        concealment_events: Option<u64>,
        frames_received: Option<u64>,
    },
}

// https://www.w3.org/TR/webrtc-stats/#inboundrtpstats-dict*
#[derive(Debug, Deserialize)]
struct RtcInboundRtpStreamStats {
    track_id: Option<String>,
    #[serde(flatten)]
    media_type: RtcInboundRtpStreamMediaType,
    bytes_received: Option<u64>,
    packets_received: Option<u64>,
    packets_lost: Option<u64>,
    jitter: Option<f64>,
    total_decode_time: Option<f32>,
    jitter_buffer_emitted_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackStat {
    #[serde(rename = "trackIdentifier")]
    track_id: Option<String>,
    remote_source: Option<bool>,
    ended: Option<bool>,
    detached: Option<bool>,
    kind: Option<String>,
    media_source_id: Option<String>,
    jitter_buffer_delay: Option<f64>,
    jitter_buffer_emitted_count: Option<f64>,
    frame_width: Option<u64>,
    frames_received: Option<f64>,
    frames_decoded: Option<f64>,
    frames_dropped: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundRtpStats {
    kind: Option<String>,
    media_type: MediaType,
    jitter: Option<f64>,
    nack_count: Option<u64>,
    packets_loss: Option<u32>,
    packets_received: Option<u64>,
    remote_id: Option<String>,
    ssrc: Option<u64>,
    transport_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutboundRtpStats {
    bitrate_mean: Option<f32>,
    bitrate_std_dev: Option<f32>,
    bytes_sent: Option<u64>,
    dropped_frames: Option<u32>,
    fir_count: Option<u32>,
    framerate_mean: Option<f32>,
    framedrate_std_dev: Option<f32>,
    framed_encoded: Option<u32>,
    kind: Option<String>,
    framed_decoded: Option<u64>,
    media_type: Option<MediaType>,
    nack_count: Option<u32>,
    packets_sent: Option<u32>,
    pli_count: Option<u32>,
    qp_sum: Option<u32>,
    remote_id: Option<String>,
    ssrc: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCandidateStat {
    network_type: Option<String>,
    address: Option<String>,
    transport_id: Option<String>,
    port: Option<u16>,
    protocol: Option<Protocol>,
    candidate_type: Option<CandidateType>,
    priority: Option<u32>,
    url: Option<String>,
    relay_protocol: Option<Protocol>,
    deleted: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCandidateStat {
    transport_id: Option<String>,
    network_type: Option<String>,
    address: Option<String>,
    port: Option<u16>,
    protocol: Option<Protocol>,
    candidate_type: Option<CandidateType>,
    priority: Option<u32>,
    url: Option<String>,
    relay_protocol: Option<Protocol>,
    deleted: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "kind")]
pub enum MediaSourceKind {
    Video {
        width: Option<u32>,
        height: Option<u32>,
        #[serde(rename = "framesPerSecond")]
        fps: Option<u32>,
    },
    Audio {
        audio_level: Option<f32>,
        total_audio_energy: Option<f32>,
        total_samples_duration: Option<f32>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSourceStat {
    #[serde(rename = "trackIdentifier")]
    track_id: Option<String>,
    #[serde(flatten)]
    kind: MediaSourceKind,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodecStat {
    payload_type: Option<u32>,
    // TODO: Parse it as MIME.
    mime_type: Option<String>,
    clock_rate: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
// TODO: Maybe extract this data somehow?
pub struct CertificateStat {
    fingerprint: Option<String>,
    // TODO: enum
    fingerprint_algorithm: Option<String>,
    base64_certificate: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IceServerStat {
    url: String,
    port: u16,
    protocol: Protocol,
    total_requests_sent: Option<u64>,
    total_responses_received: Option<u64>,
    total_round_trip_time: Option<f32>,
}
