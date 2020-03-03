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

use std::convert::TryFrom;

use medea_client_api_proto::stats::RtcStatsType;
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
pub struct RtcStats(pub Vec<RtcStatsType>);

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
            let stat: RtcStatsType =
                JsValue::from(&stat.1).into_serde().unwrap();

            stats.push(stat);

            next = iterator.next().unwrap();
        }

        RtcStats(stats)
    }
}
