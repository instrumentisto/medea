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

use wasm_bindgen::{prelude::*, JsCast};
use web_sys::RtcStats as SysRtcStats;

use crate::utils::get_property_by_name;
use crate::utils::console_error;

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
            let stat =
                RtcStatsType::try_from(&stat.1)
                    .unwrap();

            match stat {
                RtcStatsType::CandidatePair(pair) => {
                    if pair.nominated {
                        ice_pairs.push(pair);
                    }
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

        console_error(format!("{:?}", ice_pairs));

        RtcStats { ice_pairs }
    }
}

/// https://www.w3.org/TR/webrtc-stats/#rtctatstype-*
#[derive(Debug)]
enum RtcStatsType {
    Codec,
    InboundRtp,
    OutboundRtp,
    RemoteInboundRtp,
    RemoteOutboundRtp,
    MediaSoure,
    Csrc,
    PeerConnection,
    DataChannel,
    Stream,
    Track,
    Transceiver,
    Sender,
    Receiver,
    Transport,
    SctpTransport,
    CandidatePair(RtcStat<RtcIceCandidatePairStats>),
    LocalCandidate,
    RemoteCandidate,
    Certificate,
    IceServer,
    Unknown(String),
}

impl TryFrom<&SysRtcStats> for RtcStatsType {
    type Error = ();

    fn try_from(val: &SysRtcStats) -> Result<Self, Self::Error> {
        let type_ = get_property_by_name(&val, "type", |type_| {
            type_.as_string()
        })
        .unwrap();

        use RtcStatsType::*;
        let stat = match type_.as_ref() {
            "codec" => Codec,
            "local-candidate" => LocalCandidate,
            "remote-candidate" => RemoteCandidate,
            "track" => Track,
            "candidate-pair" => {
                CandidatePair(RtcIceCandidatePairStats::from(val))
            }
            "inbound-rtp" => InboundRtp,
            "outbound-rtp" => OutboundRtp,
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
            _ => Unknown(type_),
        };

        Ok(stat)
    }
}

// https://www.w3.org/TR/webrtc-stats/#candidatepair-dict*
#[derive(Debug)]
struct RtcIceCandidatePairStats {
    state: IceCandidatePairState,
    nominated: bool,
    writable: bool,
    bytes_sent: u64,
    bytes_received: u64,
    total_round_trip_time: f64,
    current_round_trip_time: Option<f64>,
    available_outgoing_bitrate: Option<u64>,
}

impl From<&SysRtcStats> for RtcIceCandidatePairStats {
    fn from(val: &SysRtcStats) -> Self {
        let state =
            get_property_by_name(&val, "state", |val| val.as_string()).unwrap();
        let nominated = get_property_by_name(&val, "nominated", |val| val.as_bool()).unwrap();
        let writable = get_property_by_name(&val, "writable", |val| val.as_bool()).unwrap();
        let bytes_sent = get_property_by_name(&val, "bytesSent", |val| val.as_f64()).unwrap() as u64;
        let bytes_received = get_property_by_name(&val, "bytesReceived", |val| val.as_f64()).unwrap() as u64;
        let total_round_trip_time = get_property_by_name(&val, "totalRoundTripTime", |val| val.as_f64()).unwrap();
        let current_round_trip_time = get_property_by_name(&val, "currentRoundTripTime", |val| val.as_f64());
        let available_outgoing_bitrate = get_property_by_name(&val, "availableOutgoingBitrate", |val| val.as_f64()).map(|val| val as u64);

        let stat = RtcIceCandidatePairStats {
            state: IceCandidatePairState::from(state.as_ref()),
            nominated,
            writable,
            bytes_sent,
            bytes_received,
            total_round_trip_time,
            current_round_trip_time,
            available_outgoing_bitrate,
        };

        stat
    }
}

// https://www.w3.org/TR/webrtc-stats/#rtcstatsicecandidatepairstate-enum
#[derive(Debug)]
enum IceCandidatePairState {
    Frozen,
    Waiting,
    Inprogress,
    Failed,
    Succeeded,
    Unknown(String),
}

impl From<&str> for IceCandidatePairState {
    fn from(state: &str) -> Self {
        use IceCandidatePairState::*;
        match state {
            "waiting" => Waiting,
            "succeeded" => Succeeded,
            "in-progress" => Inprogress,
            "frozen" => Frozen,
            "failed" => Failed,
            _ => Unknown(state.to_owned()),
        }
    }
}

//#[derive(Debug)]
//struct RTCInboundRtpStreamStats {
//    nominated: bool,
//    writable: bool,
//    bytes_sent: u64,
//    bytes_received: u64,
//    total_round_trip_time: f64,
//    current_round_trip_time: Option<f64>,
//    available_outgoing_bitrate: Option<u64>,
//}
