//! Spec is quite new atm, and is poorly adopted by UA's:
//!
//!
//! https://www.w3.org/TR/webrtc-stats/

use web_sys::RtcStats as SysRtcStats;
use wasm_bindgen::{prelude::*, JsCast};

use crate::utils::get_property_by_name;

use crate::utils::console_error;
use std::convert::TryFrom;

#[derive(Debug)]
pub struct RtcStats {
    ice_pairs: Vec<RtcStatsType>,
}

impl From<&JsValue> for RtcStats {
    fn from(stats: &JsValue) -> Self {
        let entries_fn =
            get_property_by_name(&stats, "entries", |func: JsValue| {
                Some(func.unchecked_into::<js_sys::Function>())
            }).unwrap();

        let iterator = entries_fn.call0(stats.as_ref()).unwrap().unchecked_into::<js_sys::Iterator>();

        let ice_pairs = Vec::new();

        let mut next = iterator.next().unwrap();
        while !next.done(){
            let stat = next.value();

            let stat = RtcStatsType::try_from(stat.as_ref()).unwrap();
//            console_error(&stat);

            next = iterator.next().unwrap();
        }

        RtcStats {
            ice_pairs,
        }
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
    CandidatePair(RtcIceCandidatePairStats),
    LocalCandidate,
    RemoteCandidate,
    Certificate,
    IceServer,
    Unknown(String),
}

impl TryFrom<&JsValue> for RtcStatsType {
    type Error = ();

    fn try_from(val: &JsValue) -> Result<Self, Self::Error> {
        let _type = get_property_by_name(&val, "type", |_type| {
            _type.as_string()
        }).unwrap();

        use RtcStatsType::*;
        let stat = match _type.as_ref() {
            "codec" => Codec,
            "local-candidate" => LocalCandidate,
            "remote-candidate" => RemoteCandidate,
            "track" => Track,
            "candidate-pair" => {
                CandidatePair(RtcIceCandidatePairStats::from(&val))
            },
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
            _ => Unknown(_type.to_owned()),
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
    total_round_trip_time: f32,
    current_round_trip_time: f32,
    available_outgoing_bitrate: u64,
}

impl From<&JsValue> for RtcIceCandidatePairStats {
    fn from(val: &JsValue) -> Self {
        let state = get_property_by_name(&val, "state", |val|val.as_string()).unwrap();


        let stat = RtcIceCandidatePairStats {
            state: IceCandidatePairState::from(state.as_ref()),
            nominated: false,
            writable: false,
            bytes_sent: 0,
            bytes_received: 0,
            total_round_trip_time: 0.0,
            current_round_trip_time: 0.0,
            available_outgoing_bitrate: 0
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
