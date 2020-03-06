use std::convert::TryFrom;

use derive_more::From;
use medea_client_api_proto::stats::RtcStatsType;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::RtcStats as SysRtcStats;

use crate::utils::get_property_by_name;

struct RtcStatsReportEntry(js_sys::JsString, SysRtcStats);

#[derive(Debug, From)]
pub enum RtcStatsError {
    UndefinedId,
    UndefinedStats,
    Js(JsValue),
    EntriesNotFound,
    ParseError(serde_json::Error),
}

impl TryFrom<js_sys::Array> for RtcStatsReportEntry {
    type Error = Traced<RtcStatsError>;

    fn try_from(value: js_sys::Array) -> Result<Self, Self::Error> {
        let id = value.get(0);
        let stats = value.get(1);

        if id.is_undefined() {
            return Err(tracerr::new!(RtcStatsError::UndefinedId));
        }

        if stats.is_undefined() {
            return Err(tracerr::new!(RtcStatsError::UndefinedStats));
        }

        let id = id
            .dyn_into::<js_sys::JsString>()
            .map_err(tracerr::from_and_wrap!())?;
        let stats = stats
            .dyn_into::<SysRtcStats>()
            .map_err(tracerr::from_and_wrap!())?;

        Ok(RtcStatsReportEntry(id, stats))
    }
}

/// All available [`RtcStatsType`] of `PeerConnection`.
#[derive(Debug)]
pub struct RtcStats(pub Vec<RtcStatsType>);

impl TryFrom<&JsValue> for RtcStats {
    type Error = Traced<RtcStatsError>;

    fn try_from(stats: &JsValue) -> Result<Self, Self::Error> {
        let entries_fn =
            get_property_by_name(&stats, "entries", |func: JsValue| {
                Some(func.unchecked_into::<js_sys::Function>())
            })
            .ok_or_else(|| tracerr::new!(RtcStatsError::EntriesNotFound))?;

        let iterator = entries_fn
            .call0(stats.as_ref())
            .map_err(tracerr::from_and_wrap!())?
            .unchecked_into::<js_sys::Iterator>();

        let mut stats = Vec::new();

        for stat in iterator {
            let stat = stat.map_err(tracerr::from_and_wrap!())?;
            let stat = stat.unchecked_into::<js_sys::Array>();
            let stat = RtcStatsReportEntry::try_from(stat)
                .map_err(tracerr::map_from_and_wrap!())?;
            let stat: RtcStatsType = JsValue::from(&stat.1)
                .into_serde()
                .map_err(tracerr::from_and_wrap!())?;

            stats.push(stat);
        }

        Ok(RtcStats(stats))
    }
}
