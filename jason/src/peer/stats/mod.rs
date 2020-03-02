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

#[derive(Debug)]
pub struct RtcStat<T> {
    id: String,
    timestamp: u64,
    kind: T,
}

impl<T> RtcStat<T> {
    fn new(id: String, timestamp: u64, kind: T) -> RtcStat<T> {
        RtcStat {
            id,
            timestamp,
            kind,
        }
    }
}

#[derive(Debug)]
pub struct RtcStats {
    ice_pairs: Vec<RtcStat<RtcIceCandidatePairStats>>,
}

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

        let mut ice_pairs = Vec::new();

        let mut next = iterator.next().unwrap();
        while !next.done() {
            let stat = next.value();
            let stat = stat.unchecked_into::<js_sys::Array>();
            let stat = RtcStatsReportEntry::try_from(stat).unwrap();
            let stat = RtcStatsType::try_from(&stat.1).unwrap();

            console_error(format!("Stat: {:?}", stat));

            match stat {
                RtcStatsType::CandidatePair(pair) => {
                    if pair.kind.nominated.unwrap_or_default() {
                        ice_pairs.push(pair);
                    }
                }
                RtcStatsType::OutboundRtp(stat) => {
                }
                RtcStatsType::Unknown(unknown_stat) => {
                    // print error
                }
                _ => {
                    // ignore
                }
            }

            next = iterator.next().unwrap();
        }

        RtcStats { ice_pairs }
    }
}

/// https://www.w3.org/TR/webrtc-stats/#rtctatstype-*
#[derive(Debug)]
enum RtcStatsType {
    Codec,
    InboundRtp(RtcStat<InboundRtcStats>),
    OutboundRtp(RtcStat<OutboundRtcStats>),
    RemoteInboundRtp,
    RemoteOutboundRtp,
    MediaSoure,
    Csrc,
    PeerConnection,
    DataChannel,
    Stream,
    Track(RtcStat<TrackStat>),
    Transceiver,
    Sender,
    Receiver,
    Transport,
    SctpTransport,
    CandidatePair(RtcStat<RtcIceCandidatePairStats>),
    LocalCandidate(RtcStat<LocalCandidateStat>),
    RemoteCandidate(RtcStat<RemoteCandidateStat>),
    Certificate,
    IceServer,
    Unknown(String),
}

impl TryFrom<&SysRtcStats> for RtcStatsType {
    type Error = ();

    fn try_from(val: &SysRtcStats) -> Result<Self, Self::Error> {
        use RtcStatsType::*;

        let id = get_property_by_name(&val, "id", |id| id.as_string()).unwrap();
        let timestamp = get_property_by_name(&val, "timestamp", |timestamp| {
            timestamp.as_f64().map(|timestamp| timestamp as u64)
        })
        .unwrap();
        let kind =
            get_property_by_name(&val, "type", |type_| type_.as_string())
                .unwrap();

        let kind = match kind.as_ref() {
            "codec" => Codec,
            "local-candidate" => LocalCandidate(RtcStat::new(id, timestamp, LocalCandidateStat::from(val))),
            "remote-candidate" => RemoteCandidate(RtcStat::new(id, timestamp, RemoteCandidateStat::from(val))),
            "track" => Track(RtcStat::new(id, timestamp, TrackStat::from(val))),
            "candidate-pair" => CandidatePair(RtcStat::new(
                id,
                timestamp,
                RtcIceCandidatePairStats::from(val),
            )),
            "inbound-rtp" => InboundRtp(RtcStat::new(
                id,
                timestamp,
                InboundRtcStats::from(val),
            )),
            "outbound-rtp" => OutboundRtp(RtcStat::new(
                id,
                timestamp,
                OutboundRtcStats::from(val),
            )),
            "remote-inbound-rtp" => RemoteInboundRtp,
            "remote-outbound-rtp" => RemoteOutboundRtp,
            "media-source" => MediaSoure,
            "csrc" => Csrc,
            "peer-connection" => PeerConnection,
            "data-channel" => DataChannel,
            "stream" => Stream,
            "transceiver" => Transceiver,
            "sender" => Sender,
            "receiver" => Receiver,
            "transport" => Transport,
            "sctp-transport" => SctpTransport,
            "certificate" => Certificate,
            "ice-server" => IceServer,
            _ => Unknown(kind),
        };

        Ok(kind)
    }
}

// https://www.w3.org/TR/webrtc-stats/#candidatepair-dict*
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RtcIceCandidatePairStats {
    state: Option<IceCandidatePairState>,
    nominated: Option<bool>,
    writable: Option<bool>,
    bytes_sent: Option<u64>,
    bytes_received: Option<u64>,
    total_round_trip_time: Option<f64>,
    current_round_trip_time: Option<f64>,
    available_outgoing_bitrate: Option<u64>,
}

impl From<&SysRtcStats> for RtcIceCandidatePairStats {
    fn from(val: &SysRtcStats) -> Self {
        JsValue::from(val).into_serde().unwrap()
    }
}

// https://www.w3.org/TR/webrtc-stats/#rtcstatsicecandidatepairstate-enum
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum IceCandidatePairState {
    Frozen,
    Waiting,
    Inprogress,
    Failed,
    Succeeded,
    // TODO: make it Unknown(String) with unknown state inside.
    #[serde(other)]
    Unknown,
}

// https://www.w3.org/TR/webrtc-stats/#inboundrtpstats-dict*
#[derive(Debug)]
struct RtcInboundRtpStreamStats {
    media_type: bool,
    bytes_received: u64,
    packets_received: u64,
    packets_lost: u64,
    jitter: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrackStat {
    #[serde(rename = "trackIdentifier")]
    track_id: String,
    remote_source: Option<bool>,
    ended: bool,
    detached: bool,
    kind: String,
    media_source_id: Option<String>,
    jitter_buffer_delay: Option<f64>,
    jitter_buffer_emitted_count: Option<f64>,
    frame_width: Option<u64>,
    frames_received: Option<f64>,
    frames_decoded: Option<f64>,
    frames_dropped: Option<f64>,
}

impl From<&SysRtcStats> for TrackStat {
    fn from(val: &SysRtcStats) -> Self {
        JsValue::from(val).into_serde().unwrap()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundRtcStats {
    jitter: Option<f64>,
    kind: Option<String>,
    media_type: Option<String>,
    nack_count: Option<u32>,
    packets_loss: Option<u32>,
    packets_received: Option<u32>,
    remote_id: Option<String>,
    ssrc: Option<u64>,
}

impl From<&SysRtcStats> for InboundRtcStats {
    fn from(val: &SysRtcStats) -> Self {
        JsValue::from(val).into_serde().unwrap()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutboundRtcStats {
    bitrate_mean: Option<f32>,
    bitrate_std_dev: Option<f32>,
    bytes_sent: Option<u64>,
    dropped_frames: Option<u32>,
    fir_count: Option<u32>,
    framerate_mean: Option<f32>,
    framedrate_std_dev: Option<f32>,
    framed_encoded: Option<u32>,
    kind: Option<String>,
    media_type: Option<String>,
    nack_count: Option<u32>,
    packets_sent: Option<u32>,
    pli_count: Option<u32>,
    qp_sum: Option<u32>,
    remote_id: Option<String>,
    ssrc: Option<u64>,
}

impl From<&SysRtcStats> for OutboundRtcStats {
    fn from(val: &SysRtcStats) -> Self {
        JsValue::from(val).into_serde().unwrap()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalCandidateStat {
    network_type: Option<String>,
    address: Option<String>,
    transport_id: Option<String>,
    port: Option<u16>,
    protocol: Option<String>,
    candidate_type: Option<String>,
    priority: Option<u32>,
    url: Option<String>,
    relay_protocol: Option<String>,
    deleted: Option<bool>,
}

impl From<&SysRtcStats> for LocalCandidateStat {
    fn from(val: &SysRtcStats) -> Self {
        JsValue::from(val).into_serde().unwrap()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCandidateStat {
    transport_id: Option<String>,
    network_type: Option<String>,
    address: Option<String>,
    port: Option<u16>,
    protocol: Option<String>,
    candidate_type: Option<String>,
    priority: Option<u32>,
    url: Option<String>,
    relay_protocol: Option<String>,
    deleted: Option<bool>,
}

impl From<&SysRtcStats> for RemoteCandidateStat {
    fn from(val: &SysRtcStats) -> Self {
        JsValue::from(val).into_serde().unwrap()
    }
}
